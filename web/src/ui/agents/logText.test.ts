import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import type { AgentLogRow } from "../../api/generated/AgentLogRow";
import { createI18n } from "../../i18n";
import { logLine, logTime, shortDay } from "./logText";

const NOW = new Date(2026, 9, 28, 15, 0);
const row = (patch: Partial<AgentLogRow> = {}): AgentLogRow => ({
  id: 1,
  at: new Date(2026, 9, 28, 14, 2).getTime(),
  client: "Claude Desktop",
  tool: "search",
  query: "Raft",
  outcome: "ok",
  count: 3,
  ...patch,
});
const tr = (language: "ru" | "en") => createI18n(language).t as unknown as TFunction;

describe("строка журнала", () => {
  it("русская: время · клиент · действие «запрос» · число записей со склонением", () => {
    expect(logLine(row(), NOW, "ru", tr("ru"))).toBe("14:02 · Claude Desktop · поиск «Raft» · 3 записи");
    expect(logLine(row({ count: 1 }), NOW, "ru", tr("ru"))).toBe("14:02 · Claude Desktop · поиск «Raft» · 1 запись");
    expect(logLine(row({ count: 5, query: null, tool: "get_entries" }), NOW, "ru", tr("ru"))).toBe("14:02 · Claude Desktop · чтение записей · 5 записей");
  });

  it("отказ называет причину вместо числа записей; без записей число не показывается", () => {
    expect(logLine(row({ tool: "add_entry", query: null, outcome: "read_only", count: 0 }), NOW, "ru", tr("ru"))).toBe(
      "14:02 · Claude Desktop · добавить запись · отказано: только чтение",
    );
    expect(logLine(row({ tool: "list_chats", query: null, count: 0 }), NOW, "ru", tr("ru"))).toBe("14:02 · Claude Desktop · список чатов");
    expect(logLine(row({ outcome: "something_new", count: 0 }), NOW, "ru", tr("ru"))).toContain("сбой");
  });

  it("английская и неизвестный инструмент, пустое имя клиента", () => {
    expect(logLine(row(), NOW, "en", tr("en"))).toBe("14:02 · Claude Desktop · search “Raft” · 3 entries");
    expect(logLine(row({ tool: "drop_tables", query: null, client: "" }), NOW, "ru", tr("ru"))).toBe("14:02 · неизвестный клиент · неизвестный запрос · 3 записи");
  });

  it("служебные строки сеанса: подключение, переподключение, отключение — без записей и без слова «сбой»", () => {
    const session = { query: null, count: 0 };
    expect(logLine(row({ tool: "reconnect", ...session }), NOW, "ru", tr("ru"))).toBe("14:02 · Claude Desktop · переподключение");
    expect(logLine(row({ tool: "reconnect", ...session }), NOW, "en", tr("en"))).toBe("14:02 · Claude Desktop · reconnected");
    expect(logLine(row({ tool: "disconnect", ...session }), NOW, "en", tr("en"))).toBe("14:02 · Claude Desktop · disconnected");
  });

  it("запрос с угловыми скобками и кавычками выводится как есть, без экранирования HTML", () => {
    expect(logLine(row({ query: "<b>&\"x\"</b>" }), NOW, "ru", tr("ru"))).toContain("«<b>&\"x\"</b>»");
  });

  it("время: сегодня — часы и минуты, раньше — день и время", () => {
    expect(logTime(new Date(2026, 9, 28, 9, 5).getTime(), NOW, "ru")).toBe("09:05");
    expect(logTime(new Date(2026, 9, 21, 9, 5).getTime(), NOW, "ru")).toMatch(/^21 окт\.?, 09:05$/);
    expect(shortDay("2026-03-14", NOW, "ru")).toMatch(/^14 мар/);
    expect(shortDay("2025-03-14", NOW, "ru")).toMatch(/2025/);
    expect(shortDay("мусор", NOW, "ru")).toBe("мусор");
  });
});
