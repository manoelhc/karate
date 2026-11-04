-- Fixed non-secret IDs and fictional identities. No production data.
CREATE ROLE karate_runtime_test NOLOGIN NOSUPERUSER NOBYPASSRLS;
GRANT USAGE ON SCHEMA karate TO karate_runtime_test;
GRANT SELECT ON karate.tenants, karate.memberships TO karate_runtime_test;
GRANT SELECT, INSERT, UPDATE, DELETE ON karate.projects TO karate_runtime_test;

INSERT INTO karate.tenants (tenant_id, name) VALUES
('00000000-0000-0000-0000-000000000001', 'Same tenant name'),
('00000000-0000-0000-0000-000000000002', 'Same tenant name');
INSERT INTO karate.memberships (tenant_id, issuer, subject, role) VALUES
('00000000-0000-0000-0000-000000000001', 'https://id.example', 'alice', 'owner'),
('00000000-0000-0000-0000-000000000002', 'https://id.example', 'bob', 'owner'),
('00000000-0000-0000-0000-000000000001', 'https://id.example', 'viewer', 'viewer'),
('00000000-0000-0000-0000-000000000001', 'https://id.example', 'member', 'member');
INSERT INTO karate.memberships (tenant_id, issuer, subject, role, revoked_at, expires_at) VALUES
('00000000-0000-0000-0000-000000000001', 'https://id.example', 'revoked', 'owner', now(), NULL),
('00000000-0000-0000-0000-000000000001', 'https://id.example', 'expired', 'owner', NULL, now() - interval '1 second');
INSERT INTO karate.projects (tenant_id, project_id, name) VALUES
('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000010', 'Same project name'),
('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000010', 'Same project name');

SET ROLE karate_runtime_test;
DO $$ BEGIN
    ASSERT (SELECT count(*) FROM karate.projects) = 0, 'missing context must fail closed';
    ASSERT (SELECT count(*) FROM karate.memberships) = 0, 'missing context leaks membership';
END $$;

BEGIN;
SET LOCAL karate.tenant_id = '00000000-0000-0000-0000-000000000001';
SET LOCAL karate.issuer = 'https://id.example';
SET LOCAL karate.subject = 'alice';
DO $$ DECLARE touched integer; BEGIN
    ASSERT (SELECT count(*) FROM karate.projects) = 1, 'project isolation failed';
    ASSERT (SELECT count(*) FROM karate.tenants) = 1, 'tenant isolation failed';
    ASSERT (SELECT count(*) FROM karate.memberships) = 1, 'membership isolation failed';
    UPDATE karate.projects SET name = 'forbidden' WHERE tenant_id = '00000000-0000-0000-0000-000000000002';
    GET DIAGNOSTICS touched = ROW_COUNT;
    ASSERT touched = 0, 'cross-tenant update succeeded';
    DELETE FROM karate.projects WHERE tenant_id = '00000000-0000-0000-0000-000000000002';
    GET DIAGNOSTICS touched = ROW_COUNT;
    ASSERT touched = 0, 'cross-tenant deletion succeeded';
    BEGIN
        INSERT INTO karate.projects (tenant_id, project_id, name)
        VALUES ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000011', 'forbidden');
        RAISE EXCEPTION 'cross-tenant insert succeeded';
    EXCEPTION WHEN insufficient_privilege THEN NULL; END;
    BEGIN
        UPDATE karate.projects SET tenant_id = '00000000-0000-0000-0000-000000000002', project_id = '00000000-0000-0000-0000-000000000012';
        RAISE EXCEPTION 'cross-tenant move succeeded';
    EXCEPTION WHEN insufficient_privilege THEN NULL; END;
    BEGIN
        UPDATE karate.memberships SET role = 'owner';
        RAISE EXCEPTION 'runtime can elevate memberships';
    EXCEPTION WHEN insufficient_privilege THEN NULL; END;
END $$;
COMMIT;
DO $$ BEGIN
    ASSERT (SELECT count(*) FROM karate.projects) = 0, 'transaction context leaked';
END $$;

BEGIN;
SET LOCAL karate.tenant_id = '00000000-0000-0000-0000-000000000002';
SET LOCAL karate.issuer = 'https://id.example';
SET LOCAL karate.subject = 'alice';
DO $$ BEGIN
    ASSERT (SELECT count(*) FROM karate.projects) = 0, 'tenant ID alone granted access';
END $$;
SET LOCAL karate.tenant_id = '00000000-0000-0000-0000-000000000001';
SET LOCAL karate.issuer = 'https://attacker.example';
DO $$ BEGIN
    ASSERT (SELECT count(*) FROM karate.projects) = 0, 'issuer collision granted access';
END $$;
ROLLBACK;

BEGIN;
SET LOCAL karate.tenant_id = '00000000-0000-0000-0000-000000000001';
SET LOCAL karate.issuer = 'https://id.example';
SET LOCAL karate.subject = 'viewer';
DO $$ DECLARE touched integer; BEGIN
    ASSERT (SELECT count(*) FROM karate.projects) = 1, 'viewer cannot read';
    UPDATE karate.projects SET name = 'forbidden';
    GET DIAGNOSTICS touched = ROW_COUNT;
    ASSERT touched = 0, 'viewer can write';
    DELETE FROM karate.projects;
    GET DIAGNOSTICS touched = ROW_COUNT;
    ASSERT touched = 0, 'viewer can delete';
    BEGIN
        INSERT INTO karate.projects (tenant_id, project_id, name)
        VALUES ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000011', 'forbidden');
        RAISE EXCEPTION 'viewer can insert';
    EXCEPTION WHEN insufficient_privilege THEN NULL; END;
END $$;
SET LOCAL karate.subject = 'revoked';
DO $$ BEGIN ASSERT (SELECT count(*) FROM karate.projects) = 0, 'revoked member retains access'; END $$;
SET LOCAL karate.subject = 'expired';
DO $$ BEGIN ASSERT (SELECT count(*) FROM karate.projects) = 0, 'expired member retains access'; END $$;
SET LOCAL karate.subject = 'unknown';
DO $$ BEGIN ASSERT (SELECT count(*) FROM karate.projects) = 0, 'unknown member has access'; END $$;
ROLLBACK;

BEGIN;
SET LOCAL karate.tenant_id = '00000000-0000-0000-0000-000000000001';
SET LOCAL karate.issuer = 'https://id.example';
SET LOCAL karate.subject = 'member';
INSERT INTO karate.projects (tenant_id, project_id, name)
VALUES ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000011', 'Created by member');
UPDATE karate.projects SET name = 'Durable change', revision = revision + 1
WHERE project_id = '00000000-0000-0000-0000-000000000011' AND revision = 1;
DO $$ DECLARE touched integer; BEGIN
    UPDATE karate.projects SET name = 'stale write' WHERE project_id = '00000000-0000-0000-0000-000000000011' AND revision = 1;
    GET DIAGNOSTICS touched = ROW_COUNT;
    ASSERT touched = 0, 'optimistic concurrency check failed';
END $$;
COMMIT;

BEGIN;
SET LOCAL karate.tenant_id = '00000000-0000-0000-0000-000000000001';
SET LOCAL karate.issuer = 'https://id.example';
SET LOCAL karate.subject = 'member';
DELETE FROM karate.projects;
ROLLBACK;
RESET ROLE;
DO $$ BEGIN ASSERT (SELECT count(*) FROM karate.projects) = 3, 'rollback lost acknowledged data'; END $$;

-- Current membership is evaluated again in the next statement.
UPDATE karate.memberships SET revoked_at = now() WHERE subject = 'member';
SET ROLE karate_runtime_test;
BEGIN;
SET LOCAL karate.tenant_id = '00000000-0000-0000-0000-000000000001';
SET LOCAL karate.issuer = 'https://id.example';
SET LOCAL karate.subject = 'member';
DO $$ BEGIN ASSERT (SELECT count(*) FROM karate.projects) = 0, 'new revocation ignored'; END $$;
COMMIT;
RESET ROLE;
