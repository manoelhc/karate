BEGIN;
CREATE TABLE karate.service_keys (
    tenant_id uuid NOT NULL REFERENCES karate.tenants(tenant_id),
    key_id uuid NOT NULL,
    name text NOT NULL CHECK(btrim(name)<>'' AND char_length(name)<=120 AND name !~ '[[:cntrl:]]'),
    issuer text NOT NULL,
    subject text NOT NULL,
    membership_revision bigint NOT NULL CHECK(membership_revision>0),
    scopes text[] NOT NULL CHECK(cardinality(scopes) BETWEEN 1 AND 4 AND scopes<@ARRAY['read','write','run','export']::text[] AND array_position(scopes,NULL) IS NULL),
    expires_at bigint NOT NULL,
    verifier bytea NOT NULL CHECK(octet_length(verifier)=32),
    revision bigint NOT NULL DEFAULT 1 CHECK(revision>0),
    revoked boolean NOT NULL DEFAULT false,
    PRIMARY KEY(tenant_id,key_id),
    FOREIGN KEY(tenant_id,issuer,subject) REFERENCES karate.memberships(tenant_id,issuer,subject)
);
CREATE TABLE karate.key_events (
    tenant_id uuid NOT NULL,
    event_id uuid NOT NULL,
    key_id uuid NOT NULL,
    actor_issuer text NOT NULL,
    actor_subject text NOT NULL,
    action text NOT NULL CHECK(action IN ('key.created','key.rotated','key.revoked')),
    revision bigint NOT NULL,
    occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,event_id),
    UNIQUE(tenant_id,key_id,revision),
    FOREIGN KEY(tenant_id,key_id) REFERENCES karate.service_keys(tenant_id,key_id)
);
ALTER TABLE karate.service_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.service_keys FORCE ROW LEVEL SECURITY;
ALTER TABLE karate.key_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.key_events FORCE ROW LEVEL SECURITY;
CREATE POLICY read_key_events ON karate.key_events FOR SELECT USING (
    tenant_id=nullif(current_setting('karate.tenant_id',true),'')::uuid
    AND EXISTS(SELECT 1 FROM karate.memberships m WHERE m.tenant_id=key_events.tenant_id AND m.role IN ('owner','admin'))
);
GRANT SELECT,INSERT,UPDATE(verifier,revision,revoked,membership_revision) ON karate.service_keys TO karate_membership_guard;
GRANT INSERT ON karate.key_events TO karate_membership_guard;
GRANT CREATE ON SCHEMA karate TO karate_membership_guard;

-- Credential lookup only; no membership/authority lookup before verification.
CREATE FUNCTION karate.key_verifier(p_tenant uuid,p_id uuid) RETURNS SETOF karate.service_keys
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT * FROM karate.service_keys WHERE tenant_id=p_tenant AND key_id=p_id LIMIT 1
$$;
CREATE FUNCTION karate.list_keys(p_tenant uuid,p_limit bigint,p_after uuid) RETURNS SETOF karate.service_keys
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE actor text;
BEGIN
    actor:=karate.membership_actor(p_tenant,false);
    IF p_limit IS NULL OR p_limit<1 OR p_limit>101 THEN RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation'; END IF;
    RETURN QUERY SELECT * FROM karate.service_keys WHERE tenant_id=p_tenant AND (p_after IS NULL OR key_id>p_after)
      AND (actor='owner' OR (issuer=current_setting('karate.issuer',true) AND subject=current_setting('karate.subject',true))) ORDER BY key_id LIMIT p_limit;
END $$;
CREATE FUNCTION karate.create_key(p_tenant uuid,p_id uuid,p_name text,p_scopes text[],p_expiry bigint,p_verifier bytea,p_event uuid)
RETURNS SETOF karate.service_keys LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE creator karate.memberships; result karate.service_keys;
BEGIN
    PERFORM karate.membership_actor(p_tenant,true);
    IF p_id IS NULL OR p_event IS NULL OR p_verifier IS NULL OR octet_length(p_verifier)<>32
       OR p_name IS NULL OR btrim(p_name)='' OR char_length(p_name)>120 OR p_name ~ '[[:cntrl:]]'
       OR p_expiry IS NULL OR p_expiry<=extract(epoch FROM clock_timestamp()) OR p_expiry>extract(epoch FROM clock_timestamp())+7776000 THEN
        RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation';
    END IF;
    IF p_scopes IS NULL OR cardinality(p_scopes) NOT BETWEEN 1 AND 4 OR NOT p_scopes<@ARRAY['read','write','run','export']::text[]
       OR array_position(p_scopes,NULL) IS NOT NULL OR (SELECT count(DISTINCT x) FROM unnest(p_scopes) x)<>cardinality(p_scopes) THEN
        RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden';
    END IF;
    PERFORM 1 FROM karate.service_keys WHERE tenant_id=p_tenant AND key_id=p_id;
    IF FOUND THEN RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict'; END IF;
    SELECT * INTO creator FROM karate.memberships WHERE tenant_id=p_tenant AND issuer=current_setting('karate.issuer',true) AND subject=current_setting('karate.subject',true);
    INSERT INTO karate.service_keys(tenant_id,key_id,name,issuer,subject,membership_revision,scopes,expires_at,verifier)
    VALUES(p_tenant,p_id,p_name,creator.issuer,creator.subject,creator.revision,p_scopes,p_expiry,p_verifier) RETURNING * INTO result;
    INSERT INTO karate.key_events(tenant_id,event_id,key_id,actor_issuer,actor_subject,action,revision)
    VALUES(p_tenant,p_event,p_id,creator.issuer,creator.subject,'key.created',1);
    RETURN NEXT result;
END $$;
-- NULL verifier means revoke. Rotation supplies a fresh random verifier.
CREATE FUNCTION karate.change_key(p_tenant uuid,p_id uuid,p_revision bigint,p_verifier bytea,p_event uuid)
RETURNS SETOF karate.service_keys LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE actor text; target karate.service_keys; own boolean; creator_revision bigint;
BEGIN
    actor:=karate.membership_actor(p_tenant,true);
    SELECT * INTO target FROM karate.service_keys WHERE tenant_id=p_tenant AND key_id=p_id;
    IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='K0002',MESSAGE='not found'; END IF;
    own:=target.issuer=current_setting('karate.issuer',true) AND target.subject=current_setting('karate.subject',true);
    IF (p_verifier IS NOT NULL AND NOT own) OR (NOT own AND actor<>'owner') THEN RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
    IF p_revision IS NULL OR p_revision<1 OR p_revision>=9223372036854775807 OR p_event IS NULL OR (p_verifier IS NOT NULL AND octet_length(p_verifier)<>32) THEN
        RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation';
    END IF;
    IF target.revision<>p_revision OR target.revoked OR (p_verifier IS NOT NULL AND target.expires_at<=extract(epoch FROM clock_timestamp())) THEN
        RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict';
    END IF;
    SELECT revision INTO creator_revision FROM karate.memberships WHERE tenant_id=p_tenant AND issuer=target.issuer AND subject=target.subject;
    UPDATE karate.service_keys SET verifier=coalesce(p_verifier,verifier),revoked=p_verifier IS NULL,revision=revision+1,
        membership_revision=CASE WHEN p_verifier IS NULL THEN membership_revision ELSE creator_revision END
    WHERE tenant_id=p_tenant AND key_id=p_id RETURNING * INTO target;
    INSERT INTO karate.key_events(tenant_id,event_id,key_id,actor_issuer,actor_subject,action,revision)
    VALUES(p_tenant,p_event,p_id,current_setting('karate.issuer',true),current_setting('karate.subject',true),
      CASE WHEN p_verifier IS NULL THEN 'key.revoked' ELSE 'key.rotated' END,target.revision);
    RETURN NEXT target;
END $$;
ALTER FUNCTION karate.key_verifier(uuid,uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.list_keys(uuid,bigint,uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.create_key(uuid,uuid,text,text[],bigint,bytea,uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.change_key(uuid,uuid,bigint,bytea,uuid) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.key_verifier(uuid,uuid),karate.list_keys(uuid,bigint,uuid),
    karate.create_key(uuid,uuid,text,text[],bigint,bytea,uuid),karate.change_key(uuid,uuid,bigint,bytea,uuid) FROM PUBLIC;
REVOKE CREATE ON SCHEMA karate FROM karate_membership_guard;
COMMIT;
