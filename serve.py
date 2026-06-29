#!/usr/bin/env python3
"""Statisk webserver for OpenRA Rust + telemetri-logging.

Serverer web/ pa 0.0.0.0:8080 og tar imot POST /telemetry fra nettleseren.
Hver telemetri-linje skrives bade til konsollen og til /tmp/openra-telemetry.log,
sa spilldata fra nettleseren kan leses pa maskinen.

Kjor:  python3 serve.py            # port 8080
       python3 serve.py 8088       # valgfri port som argument
       PORT=8088 python3 serve.py  # eller via miljovariabel
"""
import http.server
import os
import socketserver
import sys

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else int(os.environ.get("PORT", 8080))
WEB_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "web")
LOG = "/tmp/openra-telemetry.log"


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=WEB_DIR, **kwargs)

    def do_POST(self):
        if self.path == "/telemetry":
            length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(length).decode("utf-8", "replace")
            line = body.strip()
            with open(LOG, "a") as f:
                f.write(line + "\n")
            print("[telemetri]", line, flush=True)
            self.send_response(204)
            self.end_headers()
        else:
            self.send_error(404)

    def log_message(self, *args):
        pass  # demp vanlige tilgangslogger; vis kun telemetri


def main():
    socketserver.ThreadingTCPServer.allow_reuse_address = True
    with socketserver.ThreadingTCPServer(("0.0.0.0", PORT), Handler) as httpd:
        print(f"Serverer {WEB_DIR} pa http://0.0.0.0:{PORT}")
        print(f"Telemetri logges til {LOG}")
        httpd.serve_forever()


if __name__ == "__main__":
    main()
