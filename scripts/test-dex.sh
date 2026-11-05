#!/bin/sh
set -eu
# Run only as a child of the private PostgreSQL/TLS fixture; never reuse an IdP.
: "${KARATE_TEST_TLS_DIR:?isolated TLS fixture required}"
if command -v docker >/dev/null 2>&1; then runtime=docker; else runtime=podman; fi
command -v "$runtime" >/dev/null
case $(uname -m) in
    arm64|aarch64) digest=331e0a33d66b182302fd906c435cfcdcd2fe065cee5bd5346668e06379449e8c ;;
    x86_64) digest=ca5daa7da6be22465702e0d93ef74e064231460681093ac253e1479960c829b3 ;;
    *) printf '%s\n' 'Unsupported Dex fixture architecture' >&2; exit 1 ;;
esac
image="ghcr.io/dexidp/dex@sha256:$digest"
fixture="$KARATE_TEST_TLS_DIR/idp"
mkdir "$fixture"
name="karate-dex-$(basename "$KARATE_TEST_TLS_DIR")"
cleanup() { "$runtime" rm --force "$name" >/dev/null 2>&1 || true; }
trap cleanup EXIT HUP INT TERM
port=$(python3 - <<'PY'
import socket
with socket.socket() as s:
    s.bind(('127.0.0.1',0));print(s.getsockname()[1])
PY
)
export KARATE_TEST_DEX_ISSUER="https://localhost:$port/dex"
python3 - "$fixture" <<'PY'
import json,os,pathlib,shutil,sys
p=pathlib.Path(sys.argv[1]);tls=p.parent
for source,dest in [('tls.pem','server.pem'),('key.pem','server-key.pem')]:
    shutil.copyfile(tls/source,p/dest);(p/dest).chmod(0o644)
# The upstream synthetic connector is test-only, not a production login bypass.
config={'issuer':os.environ['KARATE_TEST_DEX_ISSUER'],'storage':{'type':'memory'},
 'web':{'https':'0.0.0.0:5556','tlsCert':'/tmp/server.pem','tlsKey':'/tmp/server-key.pem'},
 'expiry':{'idTokens':'1h'},'logger':{'level':'error','format':'json'},
 'oauth2':{'grantTypes':['authorization_code'],'responseTypes':['code'],'skipApprovalScreen':True,'pkce':{'enforce':True,'codeChallengeMethodsSupported':['S256']}},
 'staticClients':[{'id':'karate-browser','name':'Karate isolated verification','public':True,'redirectURIs':['https://karate.example/v1/auth/callback']}],
 'connectors':[{'type':'mockCallback','id':'fixture','name':'Isolated synthetic identity'}]}
(p/'config.json').write_text(json.dumps(config));(p/'config.json').chmod(0o644)
PY
"$runtime" create --name "$name" --cap-drop=ALL --security-opt=no-new-privileges --memory=256m --cpus=1 --pids-limit=128 --publish "127.0.0.1:$port:5556" --entrypoint dex "$image" serve /tmp/config.json >/dev/null
"$runtime" cp "$fixture/." "$name:/tmp/"
"$runtime" start "$name" >/dev/null
if ! python3 - <<'PY'
import os,ssl,time,urllib.request
ctx=ssl.create_default_context(cafile=os.environ['KARATE_TEST_TLS_DIR']+'/ca.pem')
url=os.environ['KARATE_TEST_DEX_ISSUER']+'/.well-known/openid-configuration'
for _ in range(100):
    try:
        with urllib.request.urlopen(url,context=ctx,timeout=1) as r:
            assert r.status==200
        break
    except Exception as exc:
        last_error=str(exc);time.sleep(.1)
else: raise SystemExit('Dex fixture startup failed: '+last_error)
print('Dex v2.46.1 pinned container is ready; TLS verification enabled.')
PY
then
    "$runtime" logs "$name" >&2
    exit 1
fi
"$@"
