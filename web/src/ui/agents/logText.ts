import type { TFunction } from "i18next";
import type { AgentLogRow } from "../../api/generated/AgentLogRow";
import type { Language } from "../../api/generated/Language";
import { formatTime } from "../feed/dates";

const SEPARATOR = " · ";

const TOOLS = {
  connect: "agent.log.tool.connect",
  disconnect: "agent.log.tool.disconnect",
  reconnect: "agent.log.tool.reconnect",
  list_chats: "agent.log.tool.list_chats",
  search: "agent.log.tool.search",
  get_entries: "agent.log.tool.get_entries",
  on_this_day: "agent.log.tool.on_this_day",
  get_entry: "agent.log.tool.get_entry",
  add_entry: "agent.log.tool.add_entry",
  append_to_last_entry: "agent.log.tool.append_to_last_entry",
  set_task_done: "agent.log.tool.set_task_done",
  // Доступ изменён через settings.json (слайс 4b): подпись по инструменту, а не по имени клиента — клиент называет себя сам.
  settings_file: "agent.log.tool.settings_file",
} as const;

const OUTCOMES = {
  disabled: "agent.log.outcome.disabled",
  read_only: "agent.log.outcome.read_only",
  not_found: "agent.log.outcome.not_found",
  no_agent_entry: "agent.log.outcome.no_agent_entry",
  not_a_task: "agent.log.outcome.not_a_task",
  invalid_argument: "agent.log.outcome.invalid_argument",
} as const;

const isTool = (tool: string): tool is keyof typeof TOOLS => tool in TOOLS;
const isOutcome = (outcome: string): outcome is keyof typeof OUTCOMES => outcome in OUTCOMES;

/** Время строки журнала: сегодня — `14:02`, раньше — `28 окт., 14:02`. */
export function logTime(at: number, now: Date, language: Language): string {
  const date = new Date(at);
  const time = formatTime(date, language);
  if (date.toDateString() === now.toDateString()) return time;
  const day = new Intl.DateTimeFormat(language, { day: "numeric", month: "short" }).format(date);
  return `${day}, ${time}`;
}

/** День записи `YYYY-MM-DD` коротко: `14 мар.` (год — только если не этот). */
export function shortDay(journalDate: string, now: Date, language: Language): string {
  const [year, month, day] = journalDate.split("-").map(Number);
  if (!year || !month || !day) return journalDate;
  const date = new Date(Date.UTC(year, month - 1, day));
  const sameYear = year === now.getFullYear();
  return new Intl.DateTimeFormat(language, { timeZone: "UTC", day: "numeric", month: "short", ...(sameYear ? {} : { year: "numeric" }) }).format(date);
}

/** Строка журнала: «14:02 · Claude Desktop · поиск «Raft» · 3 записи» или «… · отказано: только чтение». */
export function logLine(row: AgentLogRow, now: Date, language: Language, t: TFunction): string {
  const tool = t(isTool(row.tool) ? TOOLS[row.tool] : "agent.log.tool.unknown");
  const action = row.query ? t("agent.log.withQuery", { tool, query: row.query }) : tool;
  const parts = [logTime(row.at, now, language), row.client || t("agent.log.unknownClient"), action];
  if (row.outcome !== "ok") {
    parts.push(t(isOutcome(row.outcome) ? OUTCOMES[row.outcome] : "agent.log.outcome.error"));
  } else if (row.count > 0) {
    parts.push(t("agent.log.entries", { count: row.count }));
  }
  return parts.join(SEPARATOR);
}
