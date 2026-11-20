#!/usr/bin/env python3
"""A fake Telegram Bot API server for the slice 5c screens (KINIG-151, ADR-0021), on the runner's localhost only.

    fake-bot.py <folder with S0.webp ..> <port file> <request log> [delay seconds per download]

It serves one pack, `ScreensPack` (the files of the folder, drawn by `seed_demo`), answers 400 for any other name, and writes every request line
to the log with the token replaced by `<token>`, so that `docs/screens/5c/requests.txt` holds no secret. The app reaches it only because the screens
build has the `test-hooks` feature and `KINIGE_TEST_BOT_API` set. It never talks to Telegram and has no real sticker.
"""
import json
import os
import re
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

FOLDER, PORT_FILE, LOG, DELAY = sys.argv[1], sys.argv[2], sys.argv[3], float(sys.argv[4]) if len(sys.argv) > 4 else 0.0
FILES = sorted(n for n in os.listdir(FOLDER) if re.fullmatch(r"S[0-9]+\.webp", n))
LOCK = threading.Lock()


def log(line):
    with LOCK, open(LOG, "a", encoding="utf-8") as out:
        out.write(line + "\n")


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def reply(self, status, body, kind="application/json"):
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        url = urlparse(self.path)
        shown = re.sub(r"/bot[^/]+/", "/bot<token>/", url.path)
        headers = ", ".join(f"{k.lower()}" for k in self.headers.keys())
        log(f"GET {shown}{'?' + url.query if url.query else ''}  [headers: {headers}; user-agent: {self.headers.get('User-Agent')}]")
        method = url.path.rsplit("/", 1)[-1]
        if url.path.startswith("/file/"):
            name = os.path.basename(url.path)
            if name in FILES:
                time.sleep(DELAY)
                with open(os.path.join(FOLDER, name), "rb") as f:
                    return self.reply(200, f.read(), "image/webp")
            return self.reply(404, b"{}")
        query = parse_qs(url.query)
        if method == "getStickerSet":
            if query.get("name") != ["ScreensPack"]:
                return self.reply(400, json.dumps({"ok": False, "error_code": 400}).encode())
            stickers = [
                {"file_id": n[:-5], "file_unique_id": "u" + n[1:-5], "width": 512, "height": 512, "is_animated": False,
                 "is_video": False, "emoji": "\U0001F63A", "file_size": os.path.getsize(os.path.join(FOLDER, n))}
                for n in FILES
            ]
            stickers.append({"file_id": "ANIM0", "width": 512, "height": 512, "is_animated": True, "is_video": False, "emoji": "\U0001F431"})
            body = {"ok": True, "result": {"name": "ScreensPack", "title": "Screens Pack", "stickers": stickers}}
            return self.reply(200, json.dumps(body).encode())
        if method == "getFile":
            file_id = query.get("file_id", [""])[0]
            body = {"ok": True, "result": {"file_id": file_id, "file_path": f"stickers/{file_id}.webp"}}
            return self.reply(200, json.dumps(body).encode())
        return self.reply(404, b"{}")


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
with open(PORT_FILE, "w", encoding="utf-8") as f:
    f.write(str(server.server_address[1]))
server.serve_forever()
