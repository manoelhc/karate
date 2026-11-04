BEGIN;
ALTER TABLE karate.memberships
    ADD COLUMN membership_id uuid NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    ADD UNIQUE (tenant_id, membership_id);

CREATE TABLE karate.membership_events (
    tenant_id uuid NOT NULL,
    event_id uuid NOT NULL,
    membership_id uuid NOT NULL,
    actor_issuer text NOT NULL,
    actor_subject text NOT NULL,
    action text NOT NULL CHECK (action IN ('membership.role_changed','membership.revoked')),
    old_role text NOT NULL CHECK (old_role IN ('owner','admin','member','viewer')),
    new_role text NOT NULL CHECK (new_role IN ('owner','admin','member','viewer')),
    revision bigint NOT NULL CHECK (revision > 1),
    occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id,event_id),
    UNIQUE (tenant_id,membership_id,revision),
    FOREIGN KEY (tenant_id,membership_id) REFERENCES karate.memberships(tenant_id,membership_id)
);
ALTER TABLE karate.membership_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.membership_events FORCE ROW LEVEL SECURITY;
CREATE POLICY read_membership_events ON karate.membership_events FOR SELECT USING (
    tenant_id=nullif(current_setting('karate.tenant_id',true),'')::uuid
    AND EXISTS (SELECT 1 FROM karate.memberships m WHERE m.tenant_id=membership_events.tenant_id AND m.role IN ('owner','admin'))
);

-- Isolated capability owner, never grant membership in this role to runtime.
-- BYPASSRLS is needed for controlled access to other membership rows; no login.
CREATE ROLE karate_membership_guard NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION BYPASSRLS;
GRANT USAGE, CREATE ON SCHEMA karate TO karate_membership_guard;
GRANT SELECT, UPDATE (tenant_id) ON karate.tenants TO karate_membership_guard;
GRANT SELECT, UPDATE (role,revoked_at,revision) ON karate.memberships TO karate_membership_guard;
GRANT INSERT ON karate.membership_events TO karate_membership_guard;

CREATE FUNCTION karate.membership_actor(p_tenant uuid, p_lock boolean) RETURNS text
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
    RETURN actor;
END $$;

CREATE FUNCTION karate.list_memberships(p_tenant uuid, p_limit bigint, p_after uuid)
RETURNS SETOF karate.memberships LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    PERFORM karate.membership_actor(p_tenant,false);
    IF p_limit IS NULL OR p_limit<1 OR p_limit>101 THEN
        RAISE EXCEPTION USING ERRCODE='K0004', MESSAGE='validation';
    END IF;
    RETURN QUERY SELECT m.* FROM karate.memberships m WHERE m.tenant_id=p_tenant
        AND (p_after IS NULL OR m.membership_id>p_after) ORDER BY m.membership_id LIMIT p_limit;
END $$;

CREATE FUNCTION karate.membership_snapshot(p_tenant uuid, p_member uuid)
RETURNS TABLE (target karate.memberships, other_owner boolean)
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    PERFORM karate.membership_actor(p_tenant,true);
    RETURN QUERY SELECT m, EXISTS (SELECT 1 FROM karate.memberships o WHERE o.tenant_id=p_tenant
        AND o.membership_id<>p_member AND o.role='owner' AND o.revoked_at IS NULL AND o.expires_at IS NULL)
    FROM karate.memberships m WHERE m.tenant_id=p_tenant AND m.membership_id=p_member;
END $$;

CREATE FUNCTION karate.change_membership(p_tenant uuid, p_member uuid, p_revision bigint, p_role text, p_event uuid)
RETURNS SETOF karate.memberships LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE actor text; target karate.memberships; changed karate.memberships;
BEGIN
    actor:=karate.membership_actor(p_tenant,true);
    IF p_revision IS NULL OR p_revision<1 OR p_revision>=9223372036854775807
       OR (p_role IS NOT NULL AND p_role NOT IN ('owner','admin','member','viewer')) OR p_event IS NULL THEN
        RAISE EXCEPTION USING ERRCODE='K0004', MESSAGE='validation';
    END IF;
    SELECT * INTO target FROM karate.memberships WHERE tenant_id=p_tenant AND membership_id=p_member;
    IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='K0002', MESSAGE='not found'; END IF;
    IF actor='admin' AND (target.role IN ('owner','admin') OR p_role IN ('owner','admin')) THEN
        RAISE EXCEPTION USING ERRCODE='K0001', MESSAGE='forbidden';
    END IF;
    IF target.revision<>p_revision OR target.revoked_at IS NOT NULL OR target.expires_at<=clock_timestamp() THEN
        RAISE EXCEPTION USING ERRCODE='K0003', MESSAGE='conflict';
    END IF;
    IF p_role='owner' AND target.expires_at IS NOT NULL THEN
        RAISE EXCEPTION USING ERRCODE='K0005', MESSAGE='ownership required';
    END IF;
    IF target.role='owner' AND p_role IS DISTINCT FROM 'owner' AND NOT EXISTS (
        SELECT 1 FROM karate.memberships WHERE tenant_id=p_tenant AND membership_id<>p_member
        AND role='owner' AND revoked_at IS NULL AND expires_at IS NULL
    ) THEN RAISE EXCEPTION USING ERRCODE='K0005', MESSAGE='ownership required'; END IF;
    IF target.role=p_role THEN RETURN NEXT target; RETURN; END IF;
    UPDATE karate.memberships SET role=coalesce(p_role,role),
        revoked_at=CASE WHEN p_role IS NULL THEN clock_timestamp() ELSE revoked_at END,
        revision=revision+1 WHERE tenant_id=p_tenant AND membership_id=p_member RETURNING * INTO changed;
    INSERT INTO karate.membership_events(tenant_id,event_id,membership_id,actor_issuer,actor_subject,action,old_role,new_role,revision)
    VALUES(p_tenant,p_event,p_member,current_setting('karate.issuer',true),current_setting('karate.subject',true),
        CASE WHEN p_role IS NULL THEN 'membership.revoked' ELSE 'membership.role_changed' END,target.role,changed.role,changed.revision);
    RETURN NEXT changed;
END $$;

ALTER FUNCTION karate.membership_actor(uuid,boolean) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.list_memberships(uuid,bigint,uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.membership_snapshot(uuid,uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.change_membership(uuid,uuid,bigint,text,uuid) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.membership_actor(uuid,boolean), karate.list_memberships(uuid,bigint,uuid),
    karate.membership_snapshot(uuid,uuid), karate.change_membership(uuid,uuid,bigint,text,uuid) FROM PUBLIC;
REVOKE CREATE ON SCHEMA karate FROM karate_membership_guard;
COMMIT;
