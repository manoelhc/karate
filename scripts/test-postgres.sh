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
psql -X -v ON_ERROR_STOP=1 -f tests/postgres/tenant_isolation.sql
# Durable rows must survive restart; state from SET LOCAL must not survive a transaction.
pg_ctl -D "$fixture/data" -m fast -w restart -l "$fixture/server.log" >/dev/null
psql -X -v ON_ERROR_STOP=1 -f tests/postgres/restart.sql
printf '%s\n' 'PASS: tenant isolation, role restrictions, revocation, rollback, restart and transaction-context reset.'
