"""Owned fault-injection HTTPS fixture, not an OIDC provider or product runtime."""
import http.server
import json
from pathlib import Path
import ssl
import sys
import time

root, tls = map(Path, sys.argv[1:])


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_GET(self):
        # Requests contain only public discovery/key paths, never credentials.
        with (root / "requests").open("a") as log:
            log.write(self.path + "\n")
        plan = json.loads((root / "plan.json").read_text()).get(self.path, {"status": 404})
        time.sleep(plan.get("delay", 0))
        data = plan.get("body", "").encode()
        if plan.get("invalid_utf8"):
            data = b"\xff"
        try:
            self.send_response(plan.get("status", 200))
            headers = plan.get("headers", {"Content-Type": "application/json"})
            for name, value in headers.items():
                self.send_header(name, value)
            self.send_header("Connection", "close")
            if not plan.get("stream"):
                self.send_header("Content-Length", str(len(data) + plan.get("extra_length", 0)))
            self.end_headers()
            pieces = plan.get("pieces", 1)
            for piece in range(pieces):
                time.sleep(plan.get("piece_delay", 0))
                self.wfile.write(data[len(data)*piece//pieces:len(data)*(piece+1)//pieces])
                self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError, ssl.SSLError):
            pass
        self.close_connection = True


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(tls / "tls.pem", tls / "key.pem")
server.socket = context.wrap_socket(server.socket, server_side=True)
(root / "port").write_text(str(server.server_port))
server.serve_forever()
