#!/usr/bin/env python3
"""Сеанс MCP через мост `kinige-mcp` для приёмки слайса 4a: свой клиент вместо Claude Desktop, настоящие конфиги Claude не
трогаются. Запускает мост (stdio), шлёт запросы, пишет протокол «запрос → ответ» (JSON по строке) в файл.

Использование:
  scripts/mcp-session.py <путь к kinige-mcp> <путь к сокету> <файл протокола> [--chat <id чата>] [--closed-chat <id>]

Тексты записей в протокол попадают те, что отправил сам скрипт (синтетические демо-данные)."""
import json
import subprocess
import sys

bridge, socket, out = sys.argv[1:4]
extra = dict(zip(sys.argv[4::2], sys.argv[5::2]))
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
call("tools/list")
tool("list_chats")
tool("search", {"query": "Raft"})
tool("get_entries", {"from": "2000-01-01", "to": "2100-01-01"})
added = tool("add_entry", {"text": "Итоги демо-встречи: договорились про Raft", "task": True, **({"chat_id": extra["--chat"]} if "--chat" in extra else {})})
tool("append_to_last_entry", {"text": "Ещё пункт: сравнить с Paxos", **({"chat_id": extra["--chat"]} if "--chat" in extra else {})})
try:
    entry = json.loads(added["result"]["content"][0]["text"])["entry"]
    tool("set_task_done", {"id": entry["id"], "done": True})
    tool("get_entry", {"id": entry["id"]})
except (KeyError, ValueError):
    log.write("# add_entry не вернул запись (режим «только чтение» или чат не отмечен)\n")
if "--closed-chat" in extra:
    tool("add_entry", {"text": "в закрытый чат", "chat_id": extra["--closed-chat"]})
    tool("search", {"query": "секретный", "chat_id": extra["--closed-chat"]})
proc.stdin.close()
proc.wait(timeout=10)
log.close()
