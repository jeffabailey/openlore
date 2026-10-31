#!/usr/bin/env python3
"""The race shim between the release image and moto (DEVOPS lambda-smoke).

It forwards every call and logs each as one JSON line, conditional headers
included. A release image has no test faults, so a stale ETag has to come from
outside: armed on one path, the shim writes a different object straight to moto
just before it forwards the next PUT of that path. The pass's GET has gone by
then, so the PUT carries a version that is no longer current and gets 412.

Standard library only, and it runs in the job, never in the image.

    race_shim.py --port 5056 --upstream 127.0.0.1:5055 --log calls.jsonl
    curl -X POST --data '/bucket/index/index.duckdb' localhost:5056/__race_shim/arm
"""

import argparse
import http.client
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ARM_PATH = "/__race_shim/arm"
# What the smoke reads back to tell the shim's object from the pass's.
SWAPPED_OBJECT = b"the object the race shim wrote between the pass's GET and PUT"
SWAPPED_HEADERS = {
    "x-amz-meta-published-at": "2026-01-02T00:00:00Z",
    "x-amz-meta-pass-id": "race-shim",
}
NOT_FORWARDED = {"connection", "host", "keep-alive", "transfer-encoding"}


class Race:
    """The one path armed, if any. A PUT takes the arming with it, so it fires once."""

    def __init__(self):
        self._lock = threading.Lock()
        self._armed = None

    def arm(self, path):
        with self._lock:
            self._armed = path

    def fires_on(self, path):
        with self._lock:
            fires = self._armed == path
            if fires:
                self._armed = None
            return fires


class CallLog:
    def __init__(self, path):
        self._lock = threading.Lock()
        self._file = open(path, "a", encoding="utf-8")

    def write(self, call):
        with self._lock:
            self._file.write(json.dumps(call) + "\n")
            self._file.flush()


def call_upstream(upstream, method, target, body, headers):
    """One call to moto: its status, its headers and its body."""
    connection = http.client.HTTPConnection(upstream, timeout=60)
    try:
        connection.request(method, target, body=body, headers=headers)
        reply = connection.getresponse()
        return reply.status, reply.getheaders(), reply.read()
    finally:
        connection.close()


def forwarded_headers(headers):
    return {
        name: value
        for name, value in headers.items()
        if name.lower() not in NOT_FORWARDED
    }


def shim_handler(upstream, race, call_log):
    """The request handler, closed over what it needs."""

    class ShimHandler(BaseHTTPRequestHandler):
        # The SDK's PUT can ask for 100-continue, which HTTP/1.0 would ignore.
        protocol_version = "HTTP/1.1"

        def request_body(self):
            return self.rfile.read(int(self.headers.get("Content-Length") or 0))

        def swap_the_object(self, target):
            # Moto refuses an object call with no Authorization, so it goes on this PUT's.
            authorization = {"Authorization": self.headers["Authorization"]}
            status, _, _ = call_upstream(
                upstream,
                "PUT",
                target,
                SWAPPED_OBJECT,
                SWAPPED_HEADERS | authorization,
            )
            return status

        def forward(self):
            path = self.path.split("?", 1)[0]
            body = self.request_body()
            swapped = self.command == "PUT" and race.fires_on(path)
            swap_status = self.swap_the_object(path) if swapped else None
            status, headers, answer = call_upstream(
                upstream,
                self.command,
                self.path,
                body,
                forwarded_headers(self.headers),
            )
            call_log.write(
                {
                    "method": self.command,
                    "path": path,
                    "if_match": self.headers.get("If-Match"),
                    "if_none_match": self.headers.get("If-None-Match"),
                    "swapped": swapped,
                    "swap_status": swap_status,
                    "status": status,
                }
            )
            self.answer_with(status, headers, answer)

        def answer_with(self, status, headers, answer):
            self.send_response_only(status)
            kept = {name.lower(): value for name, value in headers}
            kept = forwarded_headers(kept)
            if self.command != "HEAD":
                kept["content-length"] = str(len(answer))
            for name, value in kept.items():
                self.send_header(name, value)
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(answer)

        def arm(self):
            race.arm(self.request_body().decode("utf-8"))
            self.answer_with(204, [], b"")

        def do_POST(self):
            if self.path == ARM_PATH:
                self.arm()
            else:
                self.forward()

        do_DELETE = forward
        do_GET = forward
        do_HEAD = forward
        do_PUT = forward

        def log_message(self, format, *args):
            pass  # The call log is the record; stderr would only repeat it.

    return ShimHandler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--upstream", required=True)
    parser.add_argument("--log", required=True)
    options = parser.parse_args()
    handler = shim_handler(options.upstream, Race(), CallLog(options.log))
    # On every address: the function reaches it from its own container.
    ThreadingHTTPServer(("0.0.0.0", options.port), handler).serve_forever()


if __name__ == "__main__":
    main()
