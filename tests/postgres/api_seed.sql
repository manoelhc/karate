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

GRANT SELECT ON karate.invitation_events TO karate_api_test;
GRANT EXECUTE ON FUNCTION karate.create_invitation(uuid,uuid,text,text,bigint,uuid),
    karate.list_invitations(uuid,bigint,uuid),karate.revoke_invitation(uuid,uuid,uuid),
    karate.accept_invitation(uuid,uuid,uuid) TO karate_api_test;

GRANT SELECT ON karate.key_events TO karate_api_test;
GRANT EXECUTE ON FUNCTION karate.key_verifier(uuid,uuid),karate.list_keys(uuid,bigint,uuid),
    karate.create_key(uuid,uuid,text,text[],bigint,bytea,uuid),
    karate.change_key(uuid,uuid,bigint,bytea,uuid) TO karate_api_test;

GRANT SELECT ON karate.session_events TO karate_api_test;
GRANT EXECUTE ON FUNCTION karate.begin_login(uuid,bytea,bytea,text,text),karate.consume_login(uuid,bytea,bytea),
karate.open_session(uuid,uuid,bytea,bigint),karate.session_verifier(uuid,uuid),karate.touch_session(uuid,uuid,bytea),karate.close_sessions(uuid,uuid,boolean) TO karate_api_test;
