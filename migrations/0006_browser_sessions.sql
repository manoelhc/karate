BEGIN;
CREATE TABLE karate.login_flows (
    tenant_id uuid NOT NULL REFERENCES karate.tenants(tenant_id),
    state_hash bytea NOT NULL CHECK(octet_length(state_hash)=32),
    binding_hash bytea NOT NULL CHECK(octet_length(binding_hash)=32),
    nonce text NOT NULL CHECK(length(nonce)=64),
    pkce text NOT NULL CHECK(length(pkce)=43),
    expires_at timestamptz NOT NULL DEFAULT clock_timestamp()+interval '5 minutes',
    PRIMARY KEY(tenant_id,state_hash)
);
CREATE INDEX login_flow_expiry ON karate.login_flows(tenant_id,expires_at);
CREATE TABLE karate.browser_sessions (
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL,
    issuer text NOT NULL,
    subject text NOT NULL,
    membership_revision bigint NOT NULL CHECK(membership_revision>0),
    verifier bytea NOT NULL CHECK(octet_length(verifier)=32),
    expires_at bigint NOT NULL,
    idle_until timestamptz NOT NULL DEFAULT clock_timestamp()+interval '15 minutes',
    revoked boolean NOT NULL DEFAULT false,
    PRIMARY KEY(tenant_id,session_id),
    FOREIGN KEY(tenant_id,issuer,subject) REFERENCES karate.memberships(tenant_id,issuer,subject)
);
CREATE INDEX session_identity ON karate.browser_sessions(tenant_id,issuer,subject,membership_revision,expires_at) WHERE NOT revoked;
CREATE TABLE karate.session_events (
    tenant_id uuid NOT NULL,
    event_id uuid NOT NULL,
    session_id uuid NOT NULL,
    actor_issuer text NOT NULL,
    actor_subject text NOT NULL,
    action text NOT NULL CHECK(action IN ('session.created','session.revoked')),
    occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,event_id),
    UNIQUE(tenant_id,session_id,action),
    FOREIGN KEY(tenant_id,session_id) REFERENCES karate.browser_sessions(tenant_id,session_id)
);
ALTER TABLE karate.login_flows ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.login_flows FORCE ROW LEVEL SECURITY;
ALTER TABLE karate.browser_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.browser_sessions FORCE ROW LEVEL SECURITY;
ALTER TABLE karate.session_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.session_events FORCE ROW LEVEL SECURITY;
CREATE POLICY read_session_events ON karate.session_events FOR SELECT USING (
    tenant_id=nullif(current_setting('karate.tenant_id',true),'')::uuid
    AND EXISTS(SELECT 1 FROM karate.memberships m WHERE m.tenant_id=session_events.tenant_id AND m.role IN ('owner','admin'))
);
GRANT SELECT,INSERT,DELETE ON karate.login_flows TO karate_membership_guard;
GRANT SELECT,INSERT,UPDATE(idle_until,revoked) ON karate.browser_sessions TO karate_membership_guard;
GRANT INSERT ON karate.session_events TO karate_membership_guard;
GRANT CREATE ON SCHEMA karate TO karate_membership_guard;
CREATE FUNCTION karate.begin_login(p_tenant uuid,p_state bytea,p_binding bytea,p_nonce text,p_pkce text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    IF current_setting('transaction_isolation')<>'read committed' THEN RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='isolation'; END IF;
    PERFORM 1 FROM karate.tenants WHERE tenant_id=p_tenant FOR NO KEY UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='K0002',MESSAGE='not found'; END IF;
    IF p_state IS NULL OR octet_length(p_state)<>32 OR p_binding IS NULL OR octet_length(p_binding)<>32
       OR p_nonce IS NULL OR p_nonce !~ '^[0-9a-f]{64}$' OR p_pkce IS NULL OR p_pkce !~ '^[A-Za-z0-9_-]{43}$' THEN
        RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation'; END IF;
    DELETE FROM karate.login_flows WHERE tenant_id=p_tenant AND state_hash IN (
        SELECT state_hash FROM karate.login_flows WHERE tenant_id=p_tenant AND expires_at<=clock_timestamp() LIMIT 128);
    IF (SELECT count(*) FROM (SELECT 1 FROM karate.login_flows WHERE tenant_id=p_tenant LIMIT 128) f)>=128 THEN
        RAISE EXCEPTION USING ERRCODE='K0006',MESSAGE='capacity'; END IF;
    INSERT INTO karate.login_flows(tenant_id,state_hash,binding_hash,nonce,pkce) VALUES(p_tenant,p_state,p_binding,p_nonce,p_pkce);
END $$;
CREATE FUNCTION karate.consume_login(p_tenant uuid,p_state bytea,p_binding bytea) RETURNS TABLE(nonce text,pkce text)
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    DELETE FROM karate.login_flows WHERE tenant_id=p_tenant AND state_hash=p_state AND binding_hash=p_binding AND expires_at>clock_timestamp()
    RETURNING login_flows.nonce,login_flows.pkce
$$;
CREATE FUNCTION karate.session_member(p_tenant uuid) RETURNS karate.memberships
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE actor karate.memberships;
BEGIN
    IF current_setting('transaction_isolation')<>'read committed' THEN RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='isolation'; END IF;
    IF p_tenant IS DISTINCT FROM nullif(current_setting('karate.tenant_id',true),'')::uuid THEN RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
    PERFORM 1 FROM karate.tenants WHERE tenant_id=p_tenant FOR NO KEY UPDATE;
    SELECT * INTO actor FROM karate.memberships WHERE tenant_id=p_tenant AND issuer=current_setting('karate.issuer',true)
        AND subject=current_setting('karate.subject',true) AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>clock_timestamp());
    IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
    RETURN actor;
END $$;
CREATE FUNCTION karate.open_session(p_tenant uuid,p_id uuid,p_verifier bytea,p_expiry bigint) RETURNS SETOF karate.browser_sessions
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE actor karate.memberships; result karate.browser_sessions;
BEGIN
    actor:=karate.session_member(p_tenant);
    IF p_id IS NULL OR p_verifier IS NULL OR octet_length(p_verifier)<>32 OR p_expiry IS NULL
       OR p_expiry<=extract(epoch FROM clock_timestamp()) OR p_expiry>extract(epoch FROM clock_timestamp())+3600 THEN
        RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation'; END IF;
    IF (SELECT count(*) FROM (SELECT 1 FROM karate.browser_sessions WHERE tenant_id=p_tenant AND issuer=actor.issuer AND subject=actor.subject
        AND membership_revision=actor.revision AND NOT revoked AND expires_at>extract(epoch FROM clock_timestamp()) AND idle_until>clock_timestamp() LIMIT 32) s)>=32 THEN
        RAISE EXCEPTION USING ERRCODE='K0006',MESSAGE='capacity'; END IF;
    INSERT INTO karate.browser_sessions(tenant_id,session_id,issuer,subject,membership_revision,verifier,expires_at)
        VALUES(p_tenant,p_id,actor.issuer,actor.subject,actor.revision,p_verifier,p_expiry) RETURNING * INTO result;
    INSERT INTO karate.session_events(tenant_id,event_id,session_id,actor_issuer,actor_subject,action)
        VALUES(p_tenant,gen_random_uuid(),p_id,actor.issuer,actor.subject,'session.created');
    RETURN NEXT result;
END $$;
CREATE FUNCTION karate.session_verifier(p_tenant uuid,p_id uuid) RETURNS SETOF karate.browser_sessions
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT * FROM karate.browser_sessions WHERE tenant_id=p_tenant AND session_id=p_id LIMIT 1
$$;
CREATE FUNCTION karate.touch_session(p_tenant uuid,p_id uuid,p_hash bytea) RETURNS SETOF karate.browser_sessions
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    UPDATE karate.browser_sessions SET idle_until=least(to_timestamp(expires_at),clock_timestamp()+interval '15 minutes')
    WHERE tenant_id=p_tenant AND session_id=p_id AND verifier=p_hash AND NOT revoked AND expires_at>extract(epoch FROM clock_timestamp()) AND idle_until>clock_timestamp()
    RETURNING *
$$;
CREATE FUNCTION karate.close_sessions(p_tenant uuid,p_id uuid,p_all boolean) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE actor karate.memberships; target karate.browser_sessions;
BEGIN
    actor:=karate.session_member(p_tenant);
    -- Bound to the current revision: older sessions already have no authority.
    -- Otherwise obsolete generations could crowd live sessions out of the limit.
    FOR target IN SELECT * FROM karate.browser_sessions WHERE tenant_id=p_tenant AND issuer=actor.issuer AND subject=actor.subject
        AND membership_revision=actor.revision AND NOT revoked AND expires_at>extract(epoch FROM clock_timestamp()) AND idle_until>clock_timestamp()
        AND (p_all OR session_id=p_id) ORDER BY session_id LIMIT 32 LOOP
        UPDATE karate.browser_sessions SET revoked=true WHERE tenant_id=p_tenant AND session_id=target.session_id;
        INSERT INTO karate.session_events(tenant_id,event_id,session_id,actor_issuer,actor_subject,action)
            VALUES(p_tenant,gen_random_uuid(),target.session_id,actor.issuer,actor.subject,'session.revoked');
    END LOOP;
END $$;
ALTER FUNCTION karate.begin_login(uuid,bytea,bytea,text,text) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.consume_login(uuid,bytea,bytea) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.session_member(uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.open_session(uuid,uuid,bytea,bigint) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.session_verifier(uuid,uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.touch_session(uuid,uuid,bytea) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.close_sessions(uuid,uuid,boolean) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.begin_login(uuid,bytea,bytea,text,text),karate.consume_login(uuid,bytea,bytea),karate.session_member(uuid),
    karate.open_session(uuid,uuid,bytea,bigint),karate.session_verifier(uuid,uuid),karate.touch_session(uuid,uuid,bytea),karate.close_sessions(uuid,uuid,boolean) FROM PUBLIC;
REVOKE CREATE ON SCHEMA karate FROM karate_membership_guard;
COMMIT;
