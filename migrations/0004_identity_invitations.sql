BEGIN;
CREATE TABLE karate.invitations (
    tenant_id uuid NOT NULL REFERENCES karate.tenants(tenant_id),
    invitation_id uuid NOT NULL,
    issuer text NOT NULL CHECK (char_length(issuer) BETWEEN 1 AND 2048),
    subject text NOT NULL CHECK (octet_length(subject) BETWEEN 1 AND 255),
    role text NOT NULL CHECK (role IN ('admin','member','viewer')),
    expires_at bigint NOT NULL,
    state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','accepted','revoked')),
    sponsor_subject text NOT NULL,
    sponsor_revision bigint NOT NULL,
    recipient_revision bigint,
    previous_role text,
    PRIMARY KEY(tenant_id,invitation_id)
);
CREATE TABLE karate.invitation_events (
    tenant_id uuid NOT NULL,
    event_id uuid NOT NULL,
    invitation_id uuid NOT NULL,
    actor_issuer text NOT NULL,
    actor_subject text NOT NULL,
    action text NOT NULL CHECK (action IN ('invitation.created','invitation.revoked','invitation.accepted')),
    membership_id uuid,
    membership_revision bigint,
    occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(tenant_id,event_id),
    UNIQUE(tenant_id,invitation_id,action),
    FOREIGN KEY(tenant_id,invitation_id) REFERENCES karate.invitations(tenant_id,invitation_id),
    FOREIGN KEY(tenant_id,membership_id) REFERENCES karate.memberships(tenant_id,membership_id)
);
ALTER TABLE karate.invitations ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.invitations FORCE ROW LEVEL SECURITY;
ALTER TABLE karate.invitation_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.invitation_events FORCE ROW LEVEL SECURITY;
CREATE POLICY read_invitation_events ON karate.invitation_events FOR SELECT USING (
    tenant_id=nullif(current_setting('karate.tenant_id',true),'')::uuid
    AND EXISTS(SELECT 1 FROM karate.memberships m WHERE m.tenant_id=invitation_events.tenant_id AND m.role IN ('owner','admin'))
);
GRANT SELECT,INSERT,UPDATE(state) ON karate.invitations TO karate_membership_guard;
GRANT INSERT ON karate.invitation_events TO karate_membership_guard;
GRANT INSERT,UPDATE(expires_at) ON karate.memberships TO karate_membership_guard;
GRANT CREATE ON SCHEMA karate TO karate_membership_guard;

CREATE FUNCTION karate.invitation_role(p_actor text,p_previous text,p_offered text) RETURNS void
LANGUAGE plpgsql SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    IF p_actor IS NULL OR p_actor NOT IN ('owner','admin') OR p_offered IS NULL OR p_offered NOT IN ('admin','member','viewer')
       OR (p_actor='admin' AND (p_offered='admin' OR coalesce(p_previous,'viewer') IN ('owner','admin'))) THEN
        RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden';
    END IF;
END $$;

CREATE FUNCTION karate.create_invitation(p_tenant uuid,p_id uuid,p_subject text,p_role text,p_expiry bigint,p_event uuid)
RETURNS SETOF karate.invitations LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE actor text; prior karate.memberships; sponsor karate.memberships; offer karate.invitations;
BEGIN
    actor:=karate.membership_actor(p_tenant,true);
    SELECT * INTO offer FROM karate.invitations WHERE tenant_id=p_tenant AND invitation_id=p_id;
    IF FOUND THEN
        IF offer.issuer=current_setting('karate.issuer',true) AND offer.subject=p_subject AND offer.role=p_role AND offer.expires_at=p_expiry AND offer.sponsor_subject=current_setting('karate.subject',true) THEN
            RETURN NEXT offer; RETURN;
        END IF;
        RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict';
    END IF;
    IF p_id IS NULL OR p_event IS NULL OR p_subject IS NULL OR octet_length(p_subject) NOT BETWEEN 1 AND 255
       OR p_expiry IS NULL OR p_expiry<=extract(epoch FROM clock_timestamp()) OR p_expiry>extract(epoch FROM clock_timestamp())+604800 THEN
        RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation';
    END IF;
    SELECT * INTO prior FROM karate.memberships WHERE tenant_id=p_tenant AND issuer=current_setting('karate.issuer',true) AND subject=p_subject;
    PERFORM karate.invitation_role(actor,prior.role,p_role);
    IF prior.membership_id IS NOT NULL AND prior.revoked_at IS NULL AND (prior.expires_at IS NULL OR prior.expires_at>clock_timestamp()) THEN
        RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict';
    END IF;
    SELECT * INTO sponsor FROM karate.memberships WHERE tenant_id=p_tenant AND issuer=current_setting('karate.issuer',true) AND subject=current_setting('karate.subject',true);
    INSERT INTO karate.invitations(tenant_id,invitation_id,issuer,subject,role,expires_at,sponsor_subject,sponsor_revision,recipient_revision,previous_role)
    VALUES(p_tenant,p_id,sponsor.issuer,p_subject,p_role,p_expiry,sponsor.subject,sponsor.revision,prior.revision,prior.role) RETURNING * INTO offer;
    INSERT INTO karate.invitation_events(tenant_id,event_id,invitation_id,actor_issuer,actor_subject,action)
    VALUES(p_tenant,p_event,p_id,sponsor.issuer,sponsor.subject,'invitation.created');
    RETURN NEXT offer;
END $$;

CREATE FUNCTION karate.list_invitations(p_tenant uuid,p_limit bigint,p_after uuid)
RETURNS SETOF karate.invitations LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    PERFORM karate.membership_actor(p_tenant,false);
    IF p_limit IS NULL OR p_limit<1 OR p_limit>101 THEN RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation'; END IF;
    RETURN QUERY SELECT * FROM karate.invitations WHERE tenant_id=p_tenant AND (p_after IS NULL OR invitation_id>p_after) ORDER BY invitation_id LIMIT p_limit;
END $$;

CREATE FUNCTION karate.revoke_invitation(p_tenant uuid,p_id uuid,p_event uuid)
RETURNS SETOF karate.invitations LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE actor text; offer karate.invitations;
BEGIN
    actor:=karate.membership_actor(p_tenant,true);
    SELECT * INTO offer FROM karate.invitations WHERE tenant_id=p_tenant AND invitation_id=p_id;
    IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='K0002',MESSAGE='not found'; END IF;
    PERFORM karate.invitation_role(actor,offer.previous_role,offer.role);
    IF offer.state<>'pending' OR offer.expires_at<=extract(epoch FROM clock_timestamp()) THEN RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='conflict'; END IF;
    UPDATE karate.invitations SET state='revoked' WHERE tenant_id=p_tenant AND invitation_id=p_id RETURNING * INTO offer;
    INSERT INTO karate.invitation_events(tenant_id,event_id,invitation_id,actor_issuer,actor_subject,action)
    VALUES(p_tenant,p_event,p_id,current_setting('karate.issuer',true),current_setting('karate.subject',true),'invitation.revoked');
    RETURN NEXT offer;
END $$;

CREATE FUNCTION karate.accept_invitation(p_tenant uuid,p_id uuid,p_event uuid)
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

ALTER FUNCTION karate.invitation_role(text,text,text) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.create_invitation(uuid,uuid,text,text,bigint,uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.list_invitations(uuid,bigint,uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.revoke_invitation(uuid,uuid,uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.accept_invitation(uuid,uuid,uuid) OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.invitation_role(text,text,text),karate.create_invitation(uuid,uuid,text,text,bigint,uuid),
    karate.list_invitations(uuid,bigint,uuid),karate.revoke_invitation(uuid,uuid,uuid),karate.accept_invitation(uuid,uuid,uuid) FROM PUBLIC;
REVOKE CREATE ON SCHEMA karate FROM karate_membership_guard;
COMMIT;
