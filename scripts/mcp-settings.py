#!/usr/bin/env python3
"""An MCP session through the `kinige-mcp` bridge for the acceptance of slice 5d (KINIG-160, ADR-0022): our own client reads the settings,
changes the text size, and meets every refusal. The real Claude configs are not touched.

Usage:
  scripts/mcp-settings.py <path to kinige-mcp> <path to the socket> <protocol file> <path to kinige.db>

It writes the "request -> response" protocol (one JSON per line) to the file and exits 1 when an answer is not the one the rules give.
Between legs it changes `mcp_mode` and `mcp_enabled` in the throwaway database with sqlite3, the way a person would in the settings
window: nothing inside the app switches access on, so this is the only test path. The shell scenario restores the access keys afterwards."""
import json
import sqlite3
import subprocess
import sys

bridge, socket, out, db = sys.argv[1:5]
proc = subprocess.Popen([bridge, "--socket", socket], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
log = open(out, "w", encoding="utf-8")
counter = 0
problems = []


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
    reply = call("tools/call", {"name": name, "arguments": arguments or {}})
    result = reply.get("result", {})
    text = (result.get("content") or [{}])[0].get("text", "")
    return bool(result.get("isError")), text


def note(text):
    log.write("# " + text + "\n")


def expect(label, got, want_error_prefix=None):
    is_error, text = got
    if want_error_prefix is None:
        if is_error:
            problems.append(f"{label}: expected success, got {text}")
    elif not (is_error and text.startswith(want_error_prefix)):
        problems.append(f"{label}: expected {want_error_prefix}, got {text}")
    return text


def set_key(key, value):
    with sqlite3.connect(db, timeout=5) as conn:
        conn.execute("INSERT INTO app_setting(key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value", (key, value))


call("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "Claude Desktop", "version": "1"}})
send({"jsonrpc": "2.0", "method": "notifications/initialized"})
tools = call("tools/list")["result"]["tools"]
note(f"tools/list: {len(tools)} tools in Read and write: " + ", ".join(sorted(t["name"] for t in tools)))
if len(tools) != 10:
    problems.append(f"expected 10 tools in Read and write, got {len(tools)}")

note("1. the settings an agent may know")
settings = json.loads(expect("get_settings", tool("get_settings")))
if sorted(settings) != ["agent_access", "markdown_copy", "shortcuts", "text_size", "writable"]:
    problems.append(f"get_settings keys: {sorted(settings)}")

note("2. a shortcut is the user's: refused, nothing changes (the reply is the same object as before)")
expect("shortcuts.capture", tool("set_setting", {"key": "shortcuts.capture", "value": "Ctrl+Alt+KeyJ"}), "setting_not_writable")
expect("shortcuts.clipboard", tool("set_setting", {"key": "shortcuts.clipboard", "value": "Meta+KeyV"}), "setting_not_writable")

note("3. agent access and the copy folder are the user's too")
for key, value in [
    ("agent_access.enabled", False),
    ("agent_access.mode", "read"),
    ("agent_access.chats", []),
    ("mirror.folder", "/tmp/elsewhere"),
]:
    expect(key, tool("set_setting", {"key": key, "value": value}), "setting_not_writable")

note("4. a raw database key name, a variant of a known name, a wrong value: invalid_argument")
for key in ["mcp_enabled", "mcp_mode", "mcp_chats", "mirror_dir", "shortcut_capture", "Text_Size", " text_size"]:
    expect(key, tool("set_setting", {"key": key, "value": "large"}), "invalid_argument")
expect("wrong value", tool("set_setting", {"key": "text_size", "value": "huge"}), "invalid_argument")

note("5. the one setting an agent may change: the text size; both windows switch at once")
changed = json.loads(expect("text_size large", tool("set_setting", {"key": "text_size", "value": "large"})))
if changed.get("text_size") != "large":
    problems.append(f"set_setting reply: {changed}")
if json.loads(expect("get_settings again", tool("get_settings"))).get("text_size") != "large":
    problems.append("get_settings did not show the new size")

note("6. Read only: reading works, a change is refused")
set_key("mcp_mode", "read")
expect("read mode get_settings", tool("get_settings"))
expect("read mode set_setting", tool("set_setting", {"key": "text_size", "value": "normal"}), "mcp_read_only")

note("7. access off (switched in the database, as the settings window would): every call is refused")
set_key("mcp_enabled", "0")
expect("off get_settings", tool("get_settings"), "mcp_disabled")
expect("off set_setting", tool("set_setting", {"key": "text_size", "value": "normal"}), "mcp_disabled")
set_key("mcp_enabled", "1")
set_key("mcp_mode", "read_write")

proc.stdin.close()
proc.wait(timeout=10)
for problem in problems:
    note("PROBLEM " + problem)
log.close()
for problem in problems:
    print("ERROR: " + problem)
sys.exit(1 if problems else 0)
