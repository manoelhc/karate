-- Bootstrap control-plane storage. Apply as a dedicated migration owner.
-- The runtime role must NOT own this schema, be superuser, or have BYPASSRLS.
BEGIN;
CREATE SCHEMA karate;
REVOKE ALL ON SCHEMA karate FROM PUBLIC;

CREATE TABLE karate.tenants (
    tenant_id uuid PRIMARY KEY,
    name text NOT NULL CHECK (btrim(name) <> '' AND char_length(name) <= 120 AND name !~ '[[:cntrl:]]'),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE karate.memberships (
    tenant_id uuid NOT NULL REFERENCES karate.tenants (tenant_id),
    issuer text NOT NULL CHECK (char_length(issuer) BETWEEN 1 AND 2048),
    subject text NOT NULL CHECK (char_length(subject) BETWEEN 1 AND 255),
    role text NOT NULL CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
    revoked_at timestamptz,
    expires_at timestamptz,
    PRIMARY KEY (tenant_id, issuer, subject)
);

CREATE TABLE karate.projects (
    tenant_id uuid NOT NULL REFERENCES karate.tenants (tenant_id),
    project_id uuid NOT NULL,
    name text NOT NULL CHECK (btrim(name) <> '' AND char_length(name) <= 120 AND name !~ '[[:cntrl:]]'),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, project_id)
);

-- No policy trusts tenant_id alone. Authenticated issuer/subject must also match
-- a current membership. Application code supplies these only after verification.
ALTER TABLE karate.memberships ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.memberships FORCE ROW LEVEL SECURITY;
CREATE POLICY own_current_membership ON karate.memberships FOR SELECT USING (
    tenant_id = nullif(current_setting('karate.tenant_id', true), '')::uuid
    AND issuer = current_setting('karate.issuer', true)
    AND subject = current_setting('karate.subject', true)
    AND revoked_at IS NULL
    AND (expires_at IS NULL OR expires_at > statement_timestamp())
);

ALTER TABLE karate.tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.tenants FORCE ROW LEVEL SECURITY;
CREATE POLICY member_tenant ON karate.tenants FOR SELECT USING (
    tenant_id = nullif(current_setting('karate.tenant_id', true), '')::uuid
    AND EXISTS (SELECT 1 FROM karate.memberships m WHERE m.tenant_id = tenants.tenant_id)
);

ALTER TABLE karate.projects ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.projects FORCE ROW LEVEL SECURITY;
CREATE POLICY read_projects ON karate.projects FOR SELECT USING (
    tenant_id = nullif(current_setting('karate.tenant_id', true), '')::uuid
    AND EXISTS (SELECT 1 FROM karate.memberships m WHERE m.tenant_id = projects.tenant_id)
);
CREATE POLICY create_projects ON karate.projects FOR INSERT WITH CHECK (
    tenant_id = nullif(current_setting('karate.tenant_id', true), '')::uuid
    AND EXISTS (SELECT 1 FROM karate.memberships m WHERE m.tenant_id = projects.tenant_id AND m.role IN ('owner', 'admin', 'member'))
);
CREATE POLICY update_projects ON karate.projects FOR UPDATE USING (
    tenant_id = nullif(current_setting('karate.tenant_id', true), '')::uuid
    AND EXISTS (SELECT 1 FROM karate.memberships m WHERE m.tenant_id = projects.tenant_id AND m.role IN ('owner', 'admin', 'member'))
) WITH CHECK (
    tenant_id = nullif(current_setting('karate.tenant_id', true), '')::uuid
    AND EXISTS (SELECT 1 FROM karate.memberships m WHERE m.tenant_id = projects.tenant_id AND m.role IN ('owner', 'admin', 'member'))
);
CREATE POLICY delete_projects ON karate.projects FOR DELETE USING (
    tenant_id = nullif(current_setting('karate.tenant_id', true), '')::uuid
    AND EXISTS (SELECT 1 FROM karate.memberships m WHERE m.tenant_id = projects.tenant_id AND m.role IN ('owner', 'admin', 'member'))
);
COMMIT;
