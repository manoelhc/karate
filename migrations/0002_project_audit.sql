BEGIN;
CREATE TABLE karate.audit_events (
    tenant_id uuid NOT NULL REFERENCES karate.tenants(tenant_id),
    event_id uuid NOT NULL,
    actor_issuer text NOT NULL,
    actor_subject text NOT NULL,
    action text NOT NULL CHECK (action IN ('project.created','project.updated')),
    project_id uuid NOT NULL,
    occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id,event_id),
    FOREIGN KEY (tenant_id,project_id) REFERENCES karate.projects(tenant_id,project_id)
);
ALTER TABLE karate.audit_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE karate.audit_events FORCE ROW LEVEL SECURITY;
CREATE POLICY append_own_event ON karate.audit_events FOR INSERT WITH CHECK (
    tenant_id=nullif(current_setting('karate.tenant_id',true),'')::uuid
    AND actor_issuer=current_setting('karate.issuer',true)
    AND actor_subject=current_setting('karate.subject',true)
    AND EXISTS (SELECT 1 FROM karate.memberships m WHERE m.tenant_id=audit_events.tenant_id AND m.role IN ('owner','admin','member'))
);
CREATE POLICY read_admin_events ON karate.audit_events FOR SELECT USING (
    tenant_id=nullif(current_setting('karate.tenant_id',true),'')::uuid
    AND EXISTS (SELECT 1 FROM karate.memberships m WHERE m.tenant_id=audit_events.tenant_id AND m.role IN ('owner','admin'))
);
COMMIT;
