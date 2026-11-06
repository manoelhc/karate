"""Disposable browser test proxy; no production backend or authentication logic."""
import base64
import hashlib
import http.client
import http.server
import os
from pathlib import Path
import signal
import socket
import ssl
import subprocess
import threading
import time
import urllib.request
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[2]
TLS = Path(os.environ['KARATE_TEST_TLS_DIR'])
ORIGIN = os.environ['KARATE_TEST_WEB_ORIGIN']
ISSUER = os.environ['KARATE_TEST_DEX_ISSUER']
OWNER = '00000000-0000-4000-8000-000000000091'
VIEWER = '00000000-0000-4000-8000-000000000092'
SUBJECT = base64.urlsafe_b64encode(b'\x0a\x0d0-385-28089-0\x12\x07fixture').decode().rstrip('=')
subprocess.run(['psql', '-X', '-v', 'ON_ERROR_STOP=1', '-v', f'issuer={ISSUER}', '-v', f'subject={SUBJECT}'], input=f"""
INSERT INTO karate.tenants(tenant_id,name) VALUES ('{OWNER}','Garden team'),('{VIEWER}','Read-only team');
INSERT INTO karate.memberships(tenant_id,issuer,subject,role) VALUES
('{OWNER}', :'issuer', :'subject', 'owner'), ('{VIEWER}', :'issuer', :'subject', 'viewer');
""", text=True, check=True, stdout=subprocess.DEVNULL)
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    api_port = sock.getsockname()[1]
env = dict(os.environ, KARATE_DATABASE_URL=os.environ['KARATE_TEST_DATABASE_URL'],
           KARATE_ISSUER=ISSUER, KARATE_AUDIENCE='karate-api', KARATE_OIDC_DISCOVERY='true',
           KARATE_OIDC_CA_FILE=str(TLS/'ca.pem'), KARATE_BROWSER_CLIENT_ID='karate-browser',
           KARATE_PUBLIC_ORIGIN=ORIGIN, KARATE_BIND=f'127.0.0.1:{api_port}')
api_log = (TLS/'web-api.log').open('w')
api = subprocess.Popen([os.environ.get('KARATE_TEST_API_BINARY', str(ROOT/'target/debug/karate-api'))], env=env, stdout=api_log, stderr=api_log)

class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT/'web/dist'), **kwargs)

    def log_message(self, *args):
        pass  # Never record code/state/cookies.

    def end_headers(self):
        self.send_header('Content-Security-Policy', "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'")
        self.send_header('Referrer-Policy', 'no-referrer')
        self.send_header('X-Content-Type-Options', 'nosniff')
        self.send_header('Permissions-Policy', 'camera=(), microphone=(), geolocation=()')
        self.send_header('Cache-Control', 'no-store')
        super().end_headers()

    def proxy(self):
        length = int(self.headers.get('Content-Length', '0'))
        if length > 65536:
            self.send_error(413)
            return
        body = self.rfile.read(length) if length else None
        headers = {k: v for k, v in self.headers.items() if k.lower() not in ('host','connection','transfer-encoding')}
        connection = http.client.HTTPConnection('127.0.0.1', api_port, timeout=15)
        try:
            connection.request(self.command, self.path, body, headers)
            response = connection.getresponse()
            data = response.read(65537)
            if len(data) > 65536:
                self.send_error(502)
                return
            self.send_response(response.status)
            for key, value in response.getheaders():
                if key.lower() not in ('connection','transfer-encoding','content-length','cache-control','referrer-policy'):
                    self.send_header(key, value)
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        finally:
            connection.close()

    def do_GET(self):
        if self.path.startswith('/v1/'):
            self.proxy()
        else:
            super().do_GET()

    def do_POST(self):
        self.proxy()

    def do_PATCH(self):
        self.proxy()

server = None
try:
    for _ in range(100):
        if api.poll() is not None:
            raise RuntimeError('Fixture API failed to start')
        try:
            with socket.create_connection(('127.0.0.1', api_port), timeout=.1):
                break
        except OSError:
            time.sleep(.1)
    else:
        raise RuntimeError('Fixture API startup timed out')
    server = http.server.ThreadingHTTPServer(('127.0.0.1', urlsplit(ORIGIN).port), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(TLS/'tls.pem', TLS/'key.pem')
    server.socket = context.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    # Python and Rust verify the private CA. Chromium pins only this fresh leaf SPKI.
    with urllib.request.urlopen(ORIGIN, context=ssl.create_default_context(cafile=TLS/'ca.pem')) as response:
        assert response.status == 200
    pubkey = subprocess.check_output(['openssl','x509','-in',str(TLS/'tls.pem'),'-pubkey','-noout'])
    der = subprocess.check_output(['openssl','pkey','-pubin','-outform','DER'], input=pubkey)
    env = dict(os.environ, KARATE_TEST_SPKI=base64.b64encode(hashlib.sha256(der).digest()).decode())
    result = subprocess.run([str(ROOT/'scripts/node.sh'), 'npm', '--prefix', 'web', 'run', 'e2e'], cwd=ROOT, env=env)
    raise SystemExit(result.returncode)
finally:
    if server:
        server.shutdown()
        server.server_close()
    api.send_signal(signal.SIGINT)
    try:
        api.wait(timeout=5)
    except subprocess.TimeoutExpired:
        api.kill()
        api.wait()
    api_log.close()
