#!/bin/sh
set -eu
# Allocate a loopback-only HTTPS origin before configuring the isolated Dex client.
export KARATE_TEST_WEB_ORIGIN="https://localhost:$(python3 - <<'PY'
import socket
with socket.socket() as s:
    s.bind(('127.0.0.1',0));print(s.getsockname()[1])
PY
)"
./scripts/test-postgres.sh python3 tools/web/fixture.py
