DO $$ BEGIN
    ASSERT (SELECT count(*) FROM karate.projects) = 3, 'restart lost durable rows';
    ASSERT (SELECT count(*) FROM karate.projects WHERE name = 'Durable change' AND revision = 2) = 1, 'restart lost acknowledged update';
END $$;
SET ROLE karate_runtime_test;
DO $$ BEGIN ASSERT (SELECT count(*) FROM karate.projects) = 0, 'new connection inherited authority'; END $$;
BEGIN;
SET LOCAL karate.tenant_id = '00000000-0000-0000-0000-000000000002';
SET LOCAL karate.issuer = 'https://id.example';
SET LOCAL karate.subject = 'bob';
DO $$ BEGIN ASSERT (SELECT count(*) FROM karate.projects) = 1, 'second tenant lost data'; END $$;
COMMIT;
