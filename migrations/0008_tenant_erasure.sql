BEGIN;
ALTER TABLE karate.tenant_events DROP CONSTRAINT tenant_events_action_check,
 ADD CONSTRAINT tenant_events_action_check CHECK(action IN ('tenant.created','tenant.updated','tenant.deletion_requested','tenant.deletion_cancelled')),
 ADD COLUMN deletion_request_id uuid, ADD COLUMN retention_days integer, ADD COLUMN erase_after bigint;
CREATE TABLE karate.tenant_deletions (
 tenant_id uuid PRIMARY KEY REFERENCES karate.tenants(tenant_id), request_id uuid NOT NULL,
 requester_issuer text NOT NULL, requester_subject text NOT NULL,
 state text NOT NULL CHECK(state IN ('pending','blocked_authority','erasing','cancelled')),
 requested_at bigint NOT NULL, erase_after bigint NOT NULL, retention_days integer NOT NULL CHECK(retention_days BETWEEN 7 AND 30),
 phase integer NOT NULL DEFAULT 0 CHECK(phase BETWEEN 0 AND 12)
);
CREATE INDEX deletion_due ON karate.tenant_deletions(erase_after,tenant_id) WHERE state IN ('pending','erasing');
CREATE TABLE karate.erasure_receipts (tenant_id uuid PRIMARY KEY, request_id uuid NOT NULL, completed_at bigint NOT NULL);
ALTER TABLE karate.tenant_deletions ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.tenant_deletions FORCE ROW LEVEL SECURITY;
ALTER TABLE karate.erasure_receipts ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.erasure_receipts FORCE ROW LEVEL SECURITY;
CREATE ROLE karate_erasure_guard NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION BYPASSRLS;
CREATE ROLE karate_erasure_runner NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
GRANT USAGE ON SCHEMA karate TO karate_erasure_guard,karate_erasure_runner;
GRANT SELECT,INSERT,UPDATE ON karate.tenant_deletions TO karate_membership_guard;
GRANT SELECT ON karate.erasure_receipts TO karate_membership_guard;
GRANT SELECT,DELETE ON karate.audit_events,karate.session_events,karate.key_events,karate.invitation_events,
 karate.membership_events,karate.tenant_events,karate.login_flows,karate.browser_sessions,karate.service_keys,
 karate.invitations,karate.projects,karate.memberships,karate.provisioning_grants,karate.tenants TO karate_erasure_guard;
GRANT UPDATE(tenant_id) ON karate.tenants TO karate_erasure_guard;
GRANT SELECT,UPDATE,DELETE ON karate.tenant_deletions TO karate_erasure_guard;
GRANT INSERT,SELECT ON karate.erasure_receipts TO karate_erasure_guard;
GRANT CREATE ON SCHEMA karate TO karate_membership_guard,karate_erasure_guard;

-- Interlock all lifecycle writes, plus owner browser recovery during destruction.
CREATE FUNCTION karate.deletion_state(p_tenant uuid) RETURNS text
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
 SELECT state FROM karate.tenant_deletions WHERE tenant_id=p_tenant LIMIT 1
$$;
CREATE FUNCTION karate.deletion_interlock() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM karate.tenant_deletions WHERE tenant_id=NEW.tenant_id AND state<>'cancelled') THEN
  RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='deletion pending';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER deletion_interlock BEFORE UPDATE OF name,state ON karate.tenants
 FOR EACH ROW EXECUTE FUNCTION karate.deletion_interlock();
CREATE FUNCTION karate.reject_erased_grant() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM karate.erasure_receipts WHERE tenant_id=NEW.tenant_id) THEN
  RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='tenant retired';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER reject_erased_grant BEFORE INSERT OR UPDATE ON karate.provisioning_grants
 FOR EACH ROW EXECUTE FUNCTION karate.reject_erased_grant();
CREATE TRIGGER reject_erased_tenant BEFORE INSERT ON karate.tenants
 FOR EACH ROW EXECUTE FUNCTION karate.reject_erased_grant();
CREATE FUNCTION karate.erasure_write_barrier() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
 PERFORM 1 FROM karate.tenants WHERE tenant_id=NEW.tenant_id FOR KEY SHARE;
 IF karate.deletion_state(NEW.tenant_id)='erasing' THEN RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='erasing'; END IF;
 RETURN NEW;
END $$;
DO $$ DECLARE tab text; BEGIN
 FOREACH tab IN ARRAY ARRAY['audit_events','session_events','key_events','invitation_events','membership_events','tenant_events',
 'login_flows','browser_sessions','service_keys','invitations','projects','memberships'] LOOP
  EXECUTE format('CREATE TRIGGER erasure_write_barrier BEFORE INSERT ON karate.%I FOR EACH ROW EXECUTE FUNCTION karate.erasure_write_barrier()',tab);
 END LOOP;
END $$;
CREATE FUNCTION karate.deletion_info(p_tenant uuid) RETURNS SETOF karate.tenant_deletions
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
 PERFORM karate.lock_tenant(p_tenant,false);
 IF NOT EXISTS(SELECT 1 FROM karate.memberships WHERE tenant_id=p_tenant AND issuer=current_setting('karate.issuer',true)
  AND subject=current_setting('karate.subject',true) AND role='owner' AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>clock_timestamp())) THEN
  RAISE EXCEPTION USING ERRCODE='K0001',MESSAGE='forbidden'; END IF;
 RETURN QUERY SELECT * FROM karate.tenant_deletions WHERE tenant_id=p_tenant LIMIT 1;
END $$;
CREATE FUNCTION karate.request_deletion(p_tenant uuid,p_revision bigint,p_days integer,p_cancel boolean) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE current karate.tenants; job karate.tenant_deletions; stamp bigint;
BEGIN
 SELECT * INTO current FROM karate.lock_tenant(p_tenant,true);
 PERFORM karate.deletion_info(p_tenant);
 IF p_revision IS NULL OR p_revision<1 OR p_revision=9223372036854775807 OR p_cancel IS NULL
  OR p_days IS NULL OR p_days NOT BETWEEN 7 AND 30 THEN RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='validation'; END IF;
 IF current.revision IS DISTINCT FROM p_revision THEN RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='revision'; END IF;
 SELECT * INTO job FROM karate.tenant_deletions WHERE tenant_id=p_tenant;
 IF p_cancel THEN
  IF job.state IS NULL OR job.state NOT IN ('pending','blocked_authority') THEN RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='state'; END IF;
  UPDATE karate.tenant_deletions SET state='cancelled' WHERE tenant_id=p_tenant;
 ELSE
  IF job.state IS NOT NULL AND job.state<>'cancelled' THEN RAISE EXCEPTION USING ERRCODE='K0003',MESSAGE='state'; END IF;
 END IF;
 IF NOT p_cancel THEN
  stamp:=floor(extract(epoch FROM clock_timestamp()));
  job.request_id:=gen_random_uuid();job.retention_days:=p_days;job.erase_after:=stamp+p_days::bigint*86400;
 END IF;
 -- Suspend before inserting intent so the trigger interlocks every later update.
 UPDATE karate.tenants SET state='suspended',revision=revision+1 WHERE tenant_id=p_tenant;
 INSERT INTO karate.tenant_events(tenant_id,event_id,actor_issuer,actor_subject,action,revision,state,deletion_request_id,retention_days,erase_after)
  VALUES(p_tenant,gen_random_uuid(),current_setting('karate.issuer',true),current_setting('karate.subject',true),CASE WHEN p_cancel THEN 'tenant.deletion_cancelled' ELSE 'tenant.deletion_requested' END,p_revision+1,'suspended',job.request_id,job.retention_days,job.erase_after);
 IF NOT p_cancel THEN
  INSERT INTO karate.tenant_deletions(tenant_id,request_id,requester_issuer,requester_subject,state,requested_at,erase_after,retention_days)
   VALUES(p_tenant,job.request_id,current_setting('karate.issuer',true),current_setting('karate.subject',true),'pending',stamp,job.erase_after,p_days)
   ON CONFLICT(tenant_id) DO UPDATE SET request_id=excluded.request_id,requester_issuer=excluded.requester_issuer,
    requester_subject=excluded.requester_subject,state='pending',requested_at=excluded.requested_at,erase_after=excluded.erase_after,retention_days=p_days,phase=0;
 END IF;
END $$;

CREATE FUNCTION karate.erase_next_batch() RETURNS TABLE(outcome text,deleted_rows bigint)
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE job karate.tenant_deletions; relation text; removed bigint;
 tables constant text[]:=ARRAY['audit_events','session_events','key_events','invitation_events','membership_events','tenant_events',
  'login_flows','browser_sessions','service_keys','invitations','projects','memberships','provisioning_grants'];
BEGIN
 IF current_setting('transaction_isolation')<>'read committed' THEN RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='isolation'; END IF;
 -- Select and lock tenant first: same lock order as owner request/cancel/recovery.
 SELECT d.* INTO job FROM karate.tenant_deletions d JOIN karate.tenants t USING(tenant_id)
  WHERE d.state IN ('pending','erasing') AND d.erase_after<=floor(extract(epoch FROM clock_timestamp()))
  ORDER BY d.erase_after,d.tenant_id LIMIT 1 FOR UPDATE OF t SKIP LOCKED;
 IF NOT FOUND THEN RETURN QUERY SELECT 'idle'::text,0::bigint; RETURN; END IF;
 -- Re-read after the lock under READ COMMITTED; no stale cancellation/authority.
 SELECT * INTO job FROM karate.tenant_deletions WHERE tenant_id=job.tenant_id FOR UPDATE;
 IF job.state NOT IN ('pending','erasing') OR job.erase_after>floor(extract(epoch FROM clock_timestamp())) THEN RETURN QUERY SELECT 'idle'::text,0::bigint; RETURN; END IF;
 IF job.state='pending' AND NOT EXISTS(SELECT 1 FROM karate.memberships WHERE tenant_id=job.tenant_id
  AND issuer=job.requester_issuer AND subject=job.requester_subject AND role='owner' AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>clock_timestamp())) THEN
  UPDATE karate.tenant_deletions SET state='blocked_authority' WHERE tenant_id=job.tenant_id;
  RETURN QUERY SELECT 'blocked_authority'::text,0::bigint; RETURN;
 END IF;
 IF EXISTS(SELECT 1 FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace
  WHERE n.nspname='karate' AND c.relkind IN ('r','p') AND a.attname='tenant_id' AND NOT a.attisdropped
  AND c.relname<>ALL(tables||ARRAY['tenants','tenant_deletions','erasure_receipts'])) THEN
  RAISE EXCEPTION USING ERRCODE='K0004',MESSAGE='unregistered tenant store'; END IF;
 UPDATE karate.tenant_deletions SET state='erasing' WHERE tenant_id=job.tenant_id;
 WHILE job.phase<=12 LOOP
  relation:=tables[job.phase+1];
  EXECUTE format('DELETE FROM karate.%I WHERE tenant_id=$1 AND ctid IN (SELECT ctid FROM karate.%I WHERE tenant_id=$1 LIMIT 100)',relation,relation)
   USING job.tenant_id;
  GET DIAGNOSTICS removed=ROW_COUNT;
  IF removed>0 THEN RETURN QUERY SELECT 'progress'::text,removed; RETURN; END IF;
  IF job.phase=12 THEN EXIT; END IF;
  job.phase:=job.phase+1;
  UPDATE karate.tenant_deletions SET phase=job.phase WHERE tenant_id=job.tenant_id;
 END LOOP;
 INSERT INTO karate.erasure_receipts VALUES(job.tenant_id,job.request_id,floor(extract(epoch FROM clock_timestamp())));
 DELETE FROM karate.tenant_deletions WHERE tenant_id=job.tenant_id;
 DELETE FROM karate.tenants WHERE tenant_id=job.tenant_id;
 RETURN QUERY SELECT 'primary_database_erased'::text,0::bigint;
END $$;
ALTER FUNCTION karate.erasure_write_barrier() OWNER TO karate_membership_guard;
REVOKE ALL ON FUNCTION karate.erasure_write_barrier() FROM PUBLIC;
ALTER FUNCTION karate.deletion_state(uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.deletion_interlock() OWNER TO karate_membership_guard;
ALTER FUNCTION karate.reject_erased_grant() OWNER TO karate_membership_guard;
ALTER FUNCTION karate.deletion_info(uuid) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.request_deletion(uuid,bigint,integer,boolean) OWNER TO karate_membership_guard;
ALTER FUNCTION karate.erase_next_batch() OWNER TO karate_erasure_guard;
REVOKE ALL ON FUNCTION karate.deletion_state(uuid),karate.deletion_interlock(),karate.reject_erased_grant(),karate.deletion_info(uuid),
 karate.request_deletion(uuid,bigint,integer,boolean),karate.erase_next_batch() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION karate.erase_next_batch() TO karate_erasure_runner;
REVOKE CREATE ON SCHEMA karate FROM karate_membership_guard,karate_erasure_guard;
COMMIT;
