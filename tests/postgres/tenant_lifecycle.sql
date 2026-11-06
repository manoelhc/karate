-- Isolated operator/runtime boundary checks; every row below is synthetic.
BEGIN;
SELECT karate.issue_tenant_grant('00000000-0000-0000-0000-000000000701','00000000-0000-0000-0000-000000000711',
    'Lifecycle SQL fixture','https://issuer.example','sql-owner',extract(epoch FROM clock_timestamp())::bigint+600,'isolated test');
DO $$ BEGIN
    PERFORM karate.issue_tenant_grant(gen_random_uuid(),gen_random_uuid(),'Fixture','https://issuer.example','sql-owner',0,'test');
    RAISE EXCEPTION 'expired operator grant accepted';
EXCEPTION WHEN SQLSTATE 'K0004' THEN NULL; END $$;
DO $$ BEGIN
    PERFORM karate.issue_tenant_grant('00000000-0000-0000-0000-000000000701',gen_random_uuid(),'Fixture','https://issuer.example','sql-owner',extract(epoch FROM clock_timestamp())::bigint+600,'test');
    RAISE EXCEPTION 'duplicate operator grant accepted';
EXCEPTION WHEN SQLSTATE 'K0003' THEN NULL; END $$;
CREATE ROLE karate_lifecycle_test NOLOGIN NOSUPERUSER NOBYPASSRLS;
GRANT USAGE ON SCHEMA karate TO karate_lifecycle_test;
GRANT SELECT ON karate.tenants,karate.memberships,karate.tenant_events TO karate_lifecycle_test;
GRANT SELECT,INSERT ON karate.projects TO karate_lifecycle_test;
GRANT EXECUTE ON FUNCTION karate.lock_tenant_grant(uuid,uuid),karate.provision_tenant(uuid,uuid),karate.lock_tenant(uuid,boolean),karate.tenant_active(uuid),karate.change_tenant(uuid,text,text,bigint),karate.list_memberships(uuid,bigint,uuid) TO karate_lifecycle_test;
SET LOCAL ROLE karate_lifecycle_test;
SELECT set_config('karate.tenant_id','00000000-0000-0000-0000-000000000701',true),set_config('karate.issuer','https://issuer.example',true),set_config('karate.subject','wrong-owner',true);
DO $$ BEGIN
    PERFORM karate.provision_tenant('00000000-0000-0000-0000-000000000701','00000000-0000-0000-0000-000000000711');
    RAISE EXCEPTION 'wrong grant owner accepted';
EXCEPTION WHEN SQLSTATE 'K0001' THEN NULL; END $$;
SELECT set_config('karate.subject','sql-owner',true);
DO $$ BEGIN
    PERFORM karate.provision_tenant('00000000-0000-0000-0000-000000000701','00000000-0000-0000-0000-000000000711');
    IF (SELECT count(*) FROM karate.memberships WHERE role='owner' AND expires_at IS NULL)<>1 THEN RAISE EXCEPTION 'owner bootstrap missing'; END IF;
    IF (SELECT count(*) FROM karate.tenant_events)<>1 THEN RAISE EXCEPTION 'creation audit missing'; END IF;
END $$;
DO $$ BEGIN
    PERFORM karate.provision_tenant('00000000-0000-0000-0000-000000000701','00000000-0000-0000-0000-000000000711');
    RAISE EXCEPTION 'replayed grant accepted';
EXCEPTION WHEN SQLSTATE 'K0003' THEN NULL; END $$;
INSERT INTO karate.projects(tenant_id,project_id,name) VALUES('00000000-0000-0000-0000-000000000701',gen_random_uuid(),'Retained project');
DO $$ BEGIN
    PERFORM karate.change_tenant('00000000-0000-0000-0000-000000000701','Fixture',NULL,1);
    RAISE EXCEPTION 'null state accepted';
EXCEPTION WHEN SQLSTATE 'K0004' THEN NULL; END $$;
DO $$ BEGIN
    PERFORM karate.change_tenant('00000000-0000-0000-0000-000000000701','Fixture','suspended',2);
    RAISE EXCEPTION 'stale revision accepted';
EXCEPTION WHEN SQLSTATE 'K0003' THEN NULL; END $$;
DO $$ BEGIN
    PERFORM karate.change_tenant('00000000-0000-0000-0000-000000000701','Fixture','suspended',1);
    IF (SELECT count(*) FROM karate.projects)<>0 THEN RAISE EXCEPTION 'suspended projects exposed by RLS'; END IF;
END $$;
DO $$ BEGIN
    PERFORM karate.list_memberships('00000000-0000-0000-0000-000000000701',1,NULL);
    RAISE EXCEPTION 'inactive administration accepted';
EXCEPTION WHEN SQLSTATE 'K0001' THEN NULL; END $$;
DO $$ BEGIN
    PERFORM karate.change_tenant('00000000-0000-0000-0000-000000000701','Fixture','active',2);
    IF (SELECT count(*) FROM karate.projects)<>1 THEN RAISE EXCEPTION 'retained project lost on resume'; END IF;
    PERFORM karate.change_tenant('00000000-0000-0000-0000-000000000701','Fixture','active',3);
    IF (SELECT count(*) FROM karate.tenant_events)<>3 THEN RAISE EXCEPTION 'no-op duplicated audit'; END IF;
END $$;
SELECT set_config('karate.tenant_id','00000000-0000-0000-0000-000000000702',true);
DO $$ BEGIN
    IF (SELECT count(*) FROM karate.tenant_events)<>0 THEN RAISE EXCEPTION 'cross-tenant audit leaked'; END IF;
    PERFORM karate.lock_tenant('00000000-0000-0000-0000-000000000701',true);
    RAISE EXCEPTION 'cross-tenant lock accepted';
EXCEPTION WHEN SQLSTATE 'K0001' THEN NULL; END $$;
ROLLBACK;
BEGIN ISOLATION LEVEL REPEATABLE READ;
DO $$ BEGIN
    PERFORM karate.lock_tenant(gen_random_uuid(),true);
    RAISE EXCEPTION 'stale isolation accepted';
EXCEPTION WHEN SQLSTATE 'K0004' THEN NULL; END $$;
DO $$ BEGIN
    PERFORM karate.lock_tenant_grant(gen_random_uuid(),gen_random_uuid());
    RAISE EXCEPTION 'stale grant isolation accepted';
EXCEPTION WHEN SQLSTATE 'K0004' THEN NULL; END $$;
ROLLBACK;
