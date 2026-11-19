#!/usr/bin/env python3
"""A read-only MCP session through the `kinige-mcp` bridge for the acceptance of slice 7b: an agent reads the entries of the visible chat and
searches for a renamed photo, so the protocol file shows what an agent sees of a hidden photo (its name, its description and its flag).
Our own client instead of Claude Desktop; the real Claude configs are not touched. Nothing is written to the journal.

Usage:
  scripts/mcp-read.py <path to kinige-mcp> <path to the socket> <protocol file>

The protocol holds only synthetic demo data (the screens scenario's own seed)."""
import json
import subprocess
import sys

bridge, socket, out = sys.argv[1:4]
proc = subprocess.Popen([bridge, "--socket", socket], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
log = open(out, "w", encoding="utf-8")
counter = 0


def send(message):
    log.write("-> " + json.dumps(message, ensure_ascii=False) + "\n")
    proc.stdin.write(json.dumps(message, ensure_ascii=False) + "\n")
    proc.stdin.flush()


def call(method, params=None):
    global counter
    counter += 1
    send({"jsonrpc": "2.0", "id": counter, "method": method, "params": params or {}})
    reply = json.loads(proc.stdout.readline())
    log.write("<- " + json.dumps(reply, ensure_ascii=False) + "\n")
    return reply


def tool(name, arguments=None):
    return call("tools/call", {"name": name, "arguments": arguments or {}})


call("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "Kinige acceptance script", "version": "1"}})
send({"jsonrpc": "2.0", "method": "notifications/initialized"})
tool("list_chats")
tool("get_entries", {"from": "2000-01-01", "to": "2100-01-01"})
tool("search", {"query": "закат"})
proc.stdin.close()
proc.wait(timeout=10)
log.close()
