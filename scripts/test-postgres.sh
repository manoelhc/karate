#!/bin/sh
set -eu
# Never connect to an existing database. Trust auth is restricted to a private
# Unix socket in this disposable fixture; TCP listening is disabled.
for program in initdb pg_ctl psql; do
    command -v "$program" >/dev/null || { printf '%s\n' "Missing PostgreSQL program: $program" >&2; exit 1; }
done
fixture=$(mktemp -d /tmp/karate-pg.XXXXXX)
chmod 700 "$fixture"
started=false
cleanup() {
    if [ "$started" = true ]; then pg_ctl -D "$fixture/data" -m immediate -w stop >/dev/null; fi
    # Remove only the freshly allocated test fixture, never operator databases.
    rm -rf -- "$fixture"
}
trap cleanup EXIT HUP INT TERM
initdb -D "$fixture/data" --no-locale --encoding=UTF8 --auth-local=trust --auth-host=reject -U karate_fixture >/dev/null
pg_ctl -D "$fixture/data" -l "$fixture/server.log" -o "-k $fixture -h '' -p 55439 -c max_connections=10 -c shared_buffers=16MB" -w start >/dev/null || {
    cat "$fixture/server.log" >&2; exit 1;
}
started=true
export PGHOST="$fixture" PGPORT=55439 PGUSER=karate_fixture PGDATABASE=postgres
unset PGSERVICE PGSERVICEFILE PGOPTIONS PGPASSWORD
psql -X -v ON_ERROR_STOP=1 -f migrations/0001_tenants_projects.sql
psql -X -v ON_ERROR_STOP=1 -f migrations/0002_project_audit.sql
psql -X -v ON_ERROR_STOP=1 -f migrations/0003_membership_administration.sql
psql -X -v ON_ERROR_STOP=1 -f migrations/0004_identity_invitations.sql
psql -X -v ON_ERROR_STOP=1 -f migrations/0005_scoped_api_keys.sql
psql -X -v ON_ERROR_STOP=1 -f migrations/0006_browser_sessions.sql
if [ "$#" -gt 0 ]; then
    psql -X -v ON_ERROR_STOP=1 -f tests/postgres/api_seed.sql
    openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$fixture/key.pem" 2>/dev/null
    openssl rsa -in "$fixture/key.pem" -traditional -outform DER -out "$fixture/key.der" 2>/dev/null
    openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$fixture/key2.pem" 2>/dev/null
    openssl rsa -in "$fixture/key2.pem" -traditional -outform DER -out "$fixture/key2.der" 2>/dev/null
    python3 - "$fixture" <<'PY'
import base64,json,pathlib,subprocess,sys
p=pathlib.Path(sys.argv[1])
for suffix in ('','2'):
    modulus=subprocess.check_output(['openssl','rsa','-in',str(p/f'key{suffix}.pem'),'-modulus','-noout'],stderr=subprocess.DEVNULL).decode().strip().split('=',1)[1]
    n=base64.urlsafe_b64encode(bytes.fromhex(modulus)).decode().rstrip('=')
    (p/f'jwks{suffix}.json').write_text(json.dumps({'keys':[{'kty':'RSA','alg':'RS256','use':'sig','kid':'fixture','n':n,'e':'AQAB'}]}))
PY
    openssl req -x509 -newkey rsa:2048 -nodes -keyout "$fixture/ca-key.pem" -out "$fixture/ca.pem" -days 1 -subj '/CN=Karate isolated fixture CA' -addext 'basicConstraints=critical,CA:TRUE' -addext 'keyUsage=critical,keyCertSign,cRLSign' 2>/dev/null
    openssl req -new -key "$fixture/key.pem" -out "$fixture/tls.csr" -subj '/CN=localhost' 2>/dev/null
    cat > "$fixture/tls.ext" <<'EXT'
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:localhost
EXT
    openssl x509 -req -in "$fixture/tls.csr" -CA "$fixture/ca.pem" -CAkey "$fixture/ca-key.pem" -CAcreateserial -out "$fixture/tls.pem" -days 1 -extfile "$fixture/tls.ext" 2>/dev/null
    export KARATE_TEST_TLS_DIR="$fixture"
    export KARATE_TEST_DATABASE_URL="host=$fixture port=55439 user=karate_api_test dbname=postgres"
    export KARATE_TEST_ADMIN_URL="host=$fixture port=55439 user=karate_fixture dbname=postgres"
    export KARATE_TEST_KEY="$fixture/key.der" KARATE_TEST_JWKS="$fixture/jwks.json"
    ./scripts/test-dex.sh "$@"
    exit 0
fi
psql -X -v ON_ERROR_STOP=1 -f tests/postgres/tenant_isolation.sql
# Durable rows must survive restart; state from SET LOCAL must not survive a transaction.
pg_ctl -D "$fixture/data" -m fast -w restart -l "$fixture/server.log" >/dev/null
psql -X -v ON_ERROR_STOP=1 -f tests/postgres/restart.sql
printf '%s\n' 'PASS: tenant isolation, role restrictions, revocation, rollback, restart and transaction-context reset.'
