CREATE ROLE karate_api_test LOGIN NOSUPERUSER NOBYPASSRLS;
GRANT USAGE ON SCHEMA karate TO karate_api_test;
GRANT SELECT ON karate.tenants, karate.memberships TO karate_api_test;
GRANT SELECT,INSERT,UPDATE,DELETE ON karate.projects TO karate_api_test;
GRANT SELECT,INSERT ON karate.audit_events TO karate_api_test;
GRANT SELECT ON karate.membership_events TO karate_api_test;
GRANT EXECUTE ON FUNCTION karate.list_memberships(uuid,bigint,uuid),
    karate.membership_snapshot(uuid,uuid), karate.change_membership(uuid,uuid,bigint,text,uuid) TO karate_api_test;
INSERT INTO karate.tenants(tenant_id,name) VALUES
('00000000-0000-0000-0000-000000000101','API tenant A'),
('00000000-0000-0000-0000-000000000102','API tenant B');
INSERT INTO karate.memberships(tenant_id,issuer,subject,role) VALUES
('00000000-0000-0000-0000-000000000101','https://issuer.example','alice','owner'),
('00000000-0000-0000-0000-000000000101','https://issuer.example','viewer','viewer'),
('00000000-0000-0000-0000-000000000101','https://issuer.example','member','member'),
('00000000-0000-0000-0000-000000000102','https://issuer.example','bob','owner');
