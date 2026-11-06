BEGIN;
ALTER TABLE karate.tenants ADD COLUMN state text NOT NULL DEFAULT 'active' CHECK(state IN ('active','suspended')),
    ADD COLUMN revision bigint NOT NULL DEFAULT 1 CHECK(revision>0);
CREATE TABLE karate.provisioning_grants (
    tenant_id uuid NOT NULL, grant_id uuid NOT NULL,
    name text NOT NULL CHECK(btrim(name)<>'' AND char_length(name)<=120 AND name !~ '[[:cntrl:]]'),
    issuer text NOT NULL CHECK(length(issuer) BETWEEN 1 AND 2048),
    subject text NOT NULL CHECK(length(subject) BETWEEN 1 AND 255),
    expires_at bigint NOT NULL, consumed boolean NOT NULL DEFAULT false, revoked boolean NOT NULL DEFAULT false,
    approval_reference text NOT NULL CHECK(length(approval_reference) BETWEEN 1 AND 120 AND approval_reference !~ '[[:cntrl:]]'),
    issued_by text NOT NULL DEFAULT session_user, issued_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,grant_id), UNIQUE(tenant_id)
);
CREATE TABLE karate.tenant_events (
    tenant_id uuid NOT NULL REFERENCES karate.tenants(tenant_id), event_id uuid NOT NULL,
    actor_issuer text NOT NULL, actor_subject text NOT NULL,
    action text NOT NULL CHECK(action IN ('tenant.created','tenant.updated')),
    revision bigint NOT NULL, state text NOT NULL CHECK(state IN ('active','suspended')),
    occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,event_id), UNIQUE(tenant_id,revision)
);
ALTER TABLE karate.provisioning_grants ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.provisioning_grants FORCE ROW LEVEL SECURITY;
ALTER TABLE karate.tenant_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.tenant_events FORCE ROW LEVEL SECURITY;
CREATE POLICY read_tenant_events ON karate.tenant_events FOR SELECT USING (
    tenant_id=nullif(current_setting('karate.tenant_id',true),'')::uuid
    AND EXISTS(SELECT 1 FROM karate.memberships m WHERE m.tenant_id=tenant_events.tenant_id AND m.role='owner'));
GRANT SELECT,INSERT,UPDATE ON karate.provisioning_grants TO karate_membership_guard;
GRANT INSERT,UPDATE(name,state,revision) ON karate.tenants TO karate_membership_guard;
GRANT INSERT ON karate.tenant_events TO karate_membership_guard;
GRANT CREATE ON SCHEMA karate TO karate_membership_guard;

-- Operator-only capabilities. Neither PUBLIC nor runtime receives execution.
CREATE FUNCTION karate.issue_tenant_grant(p_tenant uuid,p_grant uuid,p_name text,p_issuer text,p_subject text,p_expiry bigint,p_approval text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    IF p_tenant IS NULL OR p_grant IS NULL OR p_name IS NULL OR btrim(p_name)='' OR char_length(p_name)>120 OR p_name ~ '[[:cntrl:]]'
        OR p_issuer IS NULL OR length(p_issuer) NOT BETWEEN 1 AND 2048 OR p_subject IS NULL OR length(p_subject) NOT BETWEEN 1 AND 255
        OR p_expiry IS NULL OR p_expiry<=extract(epoch FROM clock_timestamp()) OR p_expiry>extract(epoch FROM clock_timestamp())+604800
        OR p_approval IS NULL OR length(p_approval) NOT BETWEEN 1 AND 120 OR p_approval ~ '[[:cntrl:]]' THEN
        RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation'; END IF;
    IF EXISTS(SELECT 1 FROM karate.tenants WHERE tenant_id=p_tenant) THEN RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict'; END IF;
    INSERT INTO karate.provisioning_grants(tenant_id,grant_id,name,issuer,subject,expires_at,approval_reference)
        VALUES(p_tenant,p_grant,p_name,p_issuer,p_subject,p_expiry,p_approval) ON CONFLICT(tenant_id) DO NOTHING;
    IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict'; END IF;
END $$;
CREATE FUNCTION karate.revoke_tenant_grant(p_tenant uuid,p_grant uuid) RETURNS void
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    UPDATE karate.provisioning_grants SET revoked=true WHERE tenant_id=p_tenant AND grant_id=p_grant AND NOT consumed
$$;
CREATE FUNCTION karate.lock_tenant_grant(p_tenant uuid,p_grant uuid) RETURNS SETOF karate.provisioning_grants
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    IF current_setting('transaction_isolation')<>'read committed' THEN RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='isolation'; END IF;
    IF p_tenant IS DISTINCT FROM nullif(current_setting('karate.tenant_id',true),'')::uuid THEN RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
    RETURN QUERY SELECT * FROM karate.provisioning_grants WHERE tenant_id=p_tenant AND grant_id=p_grant FOR UPDATE;
END $$;
CREATE FUNCTION karate.provision_tenant(p_tenant uuid,p_grant uuid) RETURNS SETOF karate.tenants
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE offer karate.provisioning_grants; result karate.tenants;
BEGIN
    SELECT * INTO offer FROM karate.lock_tenant_grant(p_tenant,p_grant);
    IF NOT FOUND OR offer.issuer IS DISTINCT FROM current_setting('karate.issuer',true) OR offer.subject IS DISTINCT FROM current_setting('karate.subject',true) THEN
        RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
    IF offer.consumed OR offer.revoked OR offer.expires_at<=extract(epoch FROM clock_timestamp()) OR EXISTS(SELECT 1 FROM karate.tenants WHERE tenant_id=p_tenant) THEN
        RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict'; END IF;
    INSERT INTO karate.tenants(tenant_id,name) VALUES(p_tenant,offer.name) RETURNING * INTO result;
    INSERT INTO karate.memberships(tenant_id,issuer,subject,role) VALUES(p_tenant,offer.issuer,offer.subject,'owner');
    UPDATE karate.provisioning_grants SET consumed=true WHERE tenant_id=p_tenant AND grant_id=p_grant;
    INSERT INTO karate.tenant_events(tenant_id,event_id,actor_issuer,actor_subject,action,revision,state)
        VALUES(p_tenant,gen_random_uuid(),offer.issuer,offer.subject,'tenant.created',1,'active');
    RETURN NEXT result;
END $$;
CREATE FUNCTION karate.lock_tenant(p_tenant uuid,p_exclusive boolean) RETURNS SETOF karate.tenants
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    IF current_setting('transaction_isolation')<>'read committed' THEN RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='isolation'; END IF;
    IF p_tenant IS DISTINCT FROM nullif(current_setting('karate.tenant_id',true),'')::uuid THEN RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
    IF p_exclusive THEN
        RETURN QUERY SELECT * FROM karate.tenants WHERE tenant_id=p_tenant FOR UPDATE;
    ELSE
        RETURN QUERY SELECT * FROM karate.tenants WHERE tenant_id=p_tenant FOR KEY SHARE;
    END IF;
END $$;
CREATE FUNCTION karate.tenant_active(p_tenant uuid) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT EXISTS(SELECT 1 FROM karate.tenants WHERE tenant_id=p_tenant AND state='active')
$$;
CREATE FUNCTION karate.change_tenant(p_tenant uuid,p_name text,p_state text,p_revision bigint) RETURNS SETOF karate.tenants
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE before karate.tenants; result karate.tenants;
BEGIN
    SELECT * INTO before FROM karate.lock_tenant(p_tenant,true);
    IF NOT FOUND OR NOT EXISTS(SELECT 1 FROM karate.memberships WHERE tenant_id=p_tenant AND issuer=current_setting('karate.issuer',true)
        AND subject=current_setting('karate.subject',true) AND role='owner' AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>clock_timestamp())) THEN
        RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
    IF p_name IS NULL OR btrim(p_name)='' OR char_length(p_name)>120 OR p_name ~ '[[:cntrl:]]' OR p_state IS NULL OR p_state NOT IN ('active','suspended')
        OR p_revision IS NULL OR p_revision<1 OR p_revision=9223372036854775807 THEN RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation'; END IF;
    IF before.revision<>p_revision THEN RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict'; END IF;
    IF before.name=p_name AND before.state=p_state THEN RETURN NEXT before; RETURN; END IF;
    UPDATE karate.tenants SET name=p_name,state=p_state,revision=revision+1 WHERE tenant_id=p_tenant RETURNING * INTO result;
    INSERT INTO karate.tenant_events(tenant_id,event_id,actor_issuer,actor_subject,action,revision,state)
        VALUES(p_tenant,gen_random_uuid(),current_setting('karate.issuer',true),current_setting('karate.subject',true),'tenant.updated',result.revision,result.state);
    RETURN NEXT result;
END $$;

CREATE OR REPLACE FUNCTION karate.membership_actor(p_tenant uuid, p_lock boolean) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE actor text;
BEGIN
    IF p_tenant IS DISTINCT FROM nullif(current_setting('karate.tenant_id',true),'')::uuid THEN
        RAISE EXCEPTION USING ERRCODE='K0001', MESSAGE='forbidden';
    END IF;
    -- Check before locking too: unauthorized users cannot hold another tenant lock.
    SELECT m.role INTO actor FROM karate.memberships m
    WHERE m.tenant_id=p_tenant AND m.issuer=current_setting('karate.issuer',true)
      AND m.subject=current_setting('karate.subject',true) AND m.revoked_at IS NULL
      AND (m.expires_at IS NULL OR m.expires_at>clock_timestamp());
    IF actor IS NULL OR actor NOT IN ('owner','admin') THEN
        RAISE EXCEPTION USING ERRCODE='K0001', MESSAGE='forbidden';
    END IF;
    IF p_lock THEN
        -- Post-lock authority requires a fresh snapshot; reject stronger snapshot
        -- isolation on raw SQL callers rather than allowing stale ownership facts.
        IF current_setting('transaction_isolation')<>'read committed' THEN
            RAISE EXCEPTION USING ERRCODE='K0004', MESSAGE='validation';
        END IF;
        PERFORM 1 FROM karate.tenants WHERE tenant_id=p_tenant FOR NO KEY UPDATE;
        -- Fresh statement snapshot after waiting; a revoked administrator loses authority.
        SELECT m.role INTO actor FROM karate.memberships m
        WHERE m.tenant_id=p_tenant AND m.issuer=current_setting('karate.issuer',true)
          AND m.subject=current_setting('karate.subject',true) AND m.revoked_at IS NULL
          AND (m.expires_at IS NULL OR m.expires_at>clock_timestamp());
        IF actor IS NULL OR actor NOT IN ('owner','admin') THEN
            RAISE EXCEPTION USING ERRCODE='K0001', MESSAGE='forbidden';
        END IF;
    END IF;

    IF NOT karate.tenant_active(p_tenant) THEN RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
    RETURN actor;
END $$;

CREATE OR REPLACE FUNCTION karate.accept_invitation(p_tenant uuid,p_id uuid,p_event uuid)
RETURNS TABLE (enrolled karate.memberships,sponsor_role text,previous_role text)
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE offer karate.invitations; prior karate.memberships; sponsor karate.memberships; result karate.memberships;
BEGIN
    IF p_tenant IS DISTINCT FROM nullif(current_setting('karate.tenant_id',true),'')::uuid THEN RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
    -- A locator cannot authorize an identity or let outsiders hold tenant locks.
    PERFORM 1 FROM karate.invitations WHERE tenant_id=p_tenant AND invitation_id=p_id
        AND issuer=current_setting('karate.issuer',true) AND subject=current_setting('karate.subject',true);
    IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
    IF current_setting('transaction_isolation')<>'read committed' THEN RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation'; END IF;
    PERFORM 1 FROM karate.tenants WHERE tenant_id=p_tenant FOR NO KEY UPDATE;
    IF NOT karate.tenant_active(p_tenant) THEN RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;

    SELECT * INTO offer FROM karate.invitations WHERE tenant_id=p_tenant AND invitation_id=p_id;
    IF offer.state<>'pending' OR offer.expires_at<=extract(epoch FROM clock_timestamp()) THEN RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict'; END IF;
    SELECT * INTO sponsor FROM karate.memberships WHERE tenant_id=p_tenant AND issuer=offer.issuer AND subject=offer.sponsor_subject;
    IF sponsor.membership_id IS NULL OR sponsor.revoked_at IS NOT NULL OR sponsor.expires_at<=clock_timestamp() OR sponsor.revision<>offer.sponsor_revision THEN
        RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden';
    END IF;
    SELECT * INTO prior FROM karate.memberships WHERE tenant_id=p_tenant AND issuer=offer.issuer AND subject=offer.subject;
    PERFORM karate.invitation_role(sponsor.role,prior.role,offer.role);
    IF prior.revision IS DISTINCT FROM offer.recipient_revision OR prior.revision=9223372036854775807
       OR (prior.membership_id IS NOT NULL AND prior.revoked_at IS NULL AND (prior.expires_at IS NULL OR prior.expires_at>clock_timestamp())) THEN
        RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict';
    END IF;
    IF prior.membership_id IS NULL THEN
        INSERT INTO karate.memberships(tenant_id,issuer,subject,role) VALUES(p_tenant,offer.issuer,offer.subject,offer.role) RETURNING * INTO result;
    ELSE
        UPDATE karate.memberships SET role=offer.role,revoked_at=NULL,expires_at=NULL,revision=revision+1
        WHERE tenant_id=p_tenant AND issuer=offer.issuer AND subject=offer.subject RETURNING * INTO result;
    END IF;
    UPDATE karate.invitations SET state='accepted' WHERE tenant_id=p_tenant AND invitation_id=p_id;
    INSERT INTO karate.invitation_events(tenant_id,event_id,invitation_id,actor_issuer,actor_subject,action,membership_id,membership_revision)
    VALUES(p_tenant,p_event,p_id,offer.issuer,offer.subject,'invitation.accepted',result.membership_id,result.revision);
    RETURN QUERY SELECT result,sponsor.role,prior.role;
END $$;
CREATE POLICY active_tenant ON karate.projects AS RESTRICTIVE USING (karate.tenant_active(tenant_id)) WITH CHECK (karate.tenant_active(tenant_id));
CREATE POLICY active_tenant ON karate.audit_events AS RESTRICTIVE USING (karate.tenant_active(tenant_id)) WITH CHECK (karate.tenant_active(tenant_id));
CREATE POLICY active_tenant ON karate.membership_events AS RESTRICTIVE USING (karate.tenant_active(tenant_id)) WITH CHECK (karate.tenant_active(tenant_id));
CREATE POLICY active_tenant ON karate.invitation_events AS RESTRICTIVE USING (karate.tenant_active(tenant_id)) WITH CHECK (karate.tenant_active(tenant_id));
CREATE POLICY active_tenant ON karate.key_events AS RESTRICTIVE USING (karate.tenant_active(tenant_id)) WITH CHECK (karate.tenant_active(tenant_id));
ALTER FUNCTION karate.issue_tenant_grant(uuid,uuid,text,text,text,bigint,text) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.issue_tenant_grant(uuid,uuid,text,text,text,bigint,text) FROM PUBLIC;
ALTER FUNCTION karate.revoke_tenant_grant(uuid,uuid) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.revoke_tenant_grant(uuid,uuid) FROM PUBLIC;
ALTER FUNCTION karate.lock_tenant_grant(uuid,uuid) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.lock_tenant_grant(uuid,uuid) FROM PUBLIC;
ALTER FUNCTION karate.provision_tenant(uuid,uuid) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.provision_tenant(uuid,uuid) FROM PUBLIC;
ALTER FUNCTION karate.lock_tenant(uuid,boolean) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.lock_tenant(uuid,boolean) FROM PUBLIC;
ALTER FUNCTION karate.tenant_active(uuid) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.tenant_active(uuid) FROM PUBLIC;
ALTER FUNCTION karate.change_tenant(uuid,text,text,bigint) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.change_tenant(uuid,text,text,bigint) FROM PUBLIC;
REVOKE CREATE ON SCHEMA karate FROM karate_membership_guard;
COMMIT;
