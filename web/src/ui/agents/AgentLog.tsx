import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { AgentLogEntry } from "../../api/generated/AgentLogEntry";
import type { AgentLogRow } from "../../api/generated/AgentLogRow";
import type { Language } from "../../api/generated/Language";
import { Button } from "../controls/Button";
import { Icon } from "../icons/Icon";
import { logLine, shortDay } from "./logText";

/** Журнал обращений для окна: строки, раскрытая строка с записями, какие записи уже оказались удалёнными. */
export interface AgentLogView {
  rows: readonly AgentLogRow[];
  /** Есть более старые строки. */
  hasMore: boolean;
  /** Раскрытая строка (`id`). */
  expandedId: number | null;
  /** Записи раскрытых строк; `undefined` — ещё грузятся. */
  entries: Readonly<Record<number, readonly AgentLogEntry[] | undefined>>;
  /** Id записей, которые при открытии оказались удалёнными навсегда. */
  gone: ReadonlySet<string>;
}

interface AgentLogProps {
  log: AgentLogView;
  language: Language;
  /** «Сейчас» для метки времени (в тестах и историях фиксируется). */
  now: Date;
  onToggleRow: (id: number) => void;
  onOpenEntry: (logId: number, entry: AgentLogEntry) => void;
  onMore: () => void;
}

/**
 * Журнал обращений: тихие строки «время · клиент · действие · N записей», новые сверху, внизу «Показать ещё». Имя клиента — с его
 * слов, без значка «проверено». Щелчок по строке раскрывает записи (чат, дата, начало текста); щелчок по записи открывает её
 * в ленте. Удалённая навсегда запись — тихая строка «Запись удалена». Содержимого записей в журнале нет: тексты читаются из
 * ленты по id при раскрытии.
 */
export function AgentLog({ log, language, now, onToggleRow, onOpenEntry, onMore }: AgentLogProps) {
  const { t } = useTranslation();
  const listId = useId();
  if (log.rows.length === 0) return <p className="agent-log__empty">{t("agent.log.empty")}</p>;
  return (
    <div className="agent-log">
      <ol className="agent-log__list" id={listId}>
        {log.rows.map((row) => {
          const open = log.expandedId === row.id;
          const expandable = row.count > 0;
          const text = logLine(row, now, language, t);
          const entries = log.entries[row.id];
          return (
            <li key={row.id} className="agent-log__row">
              {expandable ? (
                <button
                  type="button"
                  className="agent-log__line agent-log__line--button"
                  aria-expanded={open}
                  aria-label={`${text}. ${open ? t("agent.log.collapse") : t("agent.log.expand")}`}
                  onClick={() => onToggleRow(row.id)}
                >
                  <span className="agent-log__text">{text}</span>
                  <span className="agent-log__chevron">
                    <Icon name={open ? "chevron" : "chevronRight"} />
                  </span>
                </button>
              ) : (
                <p className="agent-log__line">
                  <span className="agent-log__text">{text}</span>
                </p>
              )}
              {open && entries ? (
                <ul className="agent-log__entries">
                  {entries.map((entry) => (
                    <LogEntry key={entry.messageId} entry={entry} gone={log.gone.has(entry.messageId)} language={language} now={now} onOpen={() => onOpenEntry(row.id, entry)} />
                  ))}
                  {row.count > entries.length ? (
                    <li className="agent-log__entry agent-log__entry--gone">
                      <span>{t("agent.log.purged", { count: row.count - entries.length })}</span>
                    </li>
                  ) : null}
                </ul>
              ) : null}
            </li>
          );
        })}
      </ol>
      {log.hasMore ? (
        <Button variant="accent" onClick={onMore}>
          {t("agent.log.more")}
        </Button>
      ) : null}
    </div>
  );
}

function LogEntry({
  entry,
  gone,
  language,
  now,
  onOpen,
}: {
  entry: AgentLogEntry;
  gone: boolean;
  language: Language;
  now: Date;
  onOpen: () => void;
}) {
  const { t } = useTranslation();
  if (!entry.message || gone) {
    return (
      <li className="agent-log__entry agent-log__entry--gone">
        <span>{t("agent.log.deleted")}</span>
      </li>
    );
  }
  const chat = entry.chatIsInbox ? t("chat.inbox") : entry.chatTitle;
  const where = `${entry.chatIcon ? `${entry.chatIcon} ` : ""}${chat} · ${shortDay(entry.message.journalDate, now, language)}`;
  return (
    <li className="agent-log__entry">
      <button type="button" className="agent-log__entry-button" onClick={onOpen}>
        <span className="agent-log__where">{where}</span>
        <span className="agent-log__snippet">{entry.message.bodyMd}</span>
      </button>
    </li>
  );
}
