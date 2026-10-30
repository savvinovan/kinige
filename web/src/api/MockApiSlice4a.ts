import { KinigeApiError } from "./KinigeApi";
import { MockApiSlice4 } from "./MockApiSlice4";
import type { AgentAccess } from "./generated/AgentAccess";
import type { AgentLogEntry } from "./generated/AgentLogEntry";
import type { AgentLogPage } from "./generated/AgentLogPage";
import type { AgentLogRow } from "./generated/AgentLogRow";
import type { AgentMode } from "./generated/AgentMode";
import type { AgentSocket } from "./generated/AgentSocket";

/** Строк журнала на странице (как `LOG_PAGE` в Rust). */
export const MOCK_LOG_PAGE = 30;

const BRIDGE = "/Applications/Kinige.app/Contents/MacOS/kinige-mcp";
const SOCKET = "/Users/me/Library/Application Support/me.savvinov.kinige/mcp/kinige.sock";

/** Состояние сокета, фрагмент для Claude Desktop и команда для Claude Code, как их присылает Rust. */
export const MOCK_AGENT_SOCKET: AgentSocket = {
  listening: true,
  error: null,
  bridgePath: BRIDGE,
  socketPath: SOCKET,
  desktopConfig: JSON.stringify({ mcpServers: { kinige: { command: BRIDGE, args: ["--socket", SOCKET] } } }, null, 2),
  codeCommand: `claude mcp add --scope user kinige -- ${BRIDGE} --socket '${SOCKET}'`,
};

/**
 * Доступ ИИ-агентов в `MockApi` (слайс 4a): флажок, режим, видимые чаты (`ai_excluded` не принимаются, как в Rust), журнал
 * обращений страницами и записи строки. Журнал и записи строк тест наполняет через `addAgentLog`.
 */
export abstract class MockApiSlice4a extends MockApiSlice4 {
  /** Событие «настройки изменились» (правка окна): его слушают окна, реализация — в `MockApiSlice4b`. */
  abstract emitSettingsChanged(): void;

  agentEnabled = false;
  agentMode: AgentMode = "read";
  /** Id чатов, отмеченных «видно агенту». */
  agentChats: string[] = [];
  /** Чаты с `ai_excluded` (в моке чатов этого признака нет). */
  readonly agentExcluded = new Set<string>();
  agentSocket: AgentSocket = MOCK_AGENT_SOCKET;
  private logRows: AgentLogRow[] = [];
  private readonly logEntries = new Map<number, AgentLogEntry[]>();

  /** Новая строка журнала (старые строки — раньше по вызову). Возвращает её `id`. */
  addAgentLog(row: Partial<AgentLogRow> & Pick<AgentLogRow, "tool">, entries: AgentLogEntry[] = []): number {
    const id = (this.logRows[0]?.id ?? 0) + 1;
    this.logRows = [
      {
        id,
        at: Date.UTC(2026, 9, 28, 11, 2),
        client: "Claude Desktop",
        query: null,
        outcome: "ok",
        count: entries.length,
        ...row,
      },
      ...this.logRows,
    ];
    this.logEntries.set(id, entries);
    return id;
  }

  getAgentAccess(): Promise<AgentAccess> {
    return this.run("getAgentAccess", "", () => ({
      enabled: this.agentEnabled,
      mode: this.agentMode,
      chats: this.chats.map((chat) => {
        const aiExcluded = this.agentExcluded.has(chat.id);
        return {
          id: chat.id,
          title: chat.title,
          icon: chat.icon,
          isInbox: chat.isInbox,
          count: chat.count,
          aiExcluded,
          visible: !aiExcluded && this.agentChats.includes(chat.id),
        };
      }),
      socket: this.agentSocket,
    }));
  }

  setAgentEnabled(enabled: boolean): Promise<void> {
    return this.run("setAgentEnabled", String(enabled), () => {
      this.agentEnabled = enabled;
      this.emitSettingsChanged();
    });
  }

  setAgentMode(mode: AgentMode): Promise<void> {
    return this.run("setAgentMode", mode, () => {
      this.agentMode = mode;
      this.emitSettingsChanged();
    });
  }

  setAgentChats(chatIds: readonly string[]): Promise<void> {
    return this.run("setAgentChats", chatIds.join(","), () => {
      for (const id of chatIds) {
        if (this.agentExcluded.has(id) || !this.chats.some((chat) => chat.id === id)) throw new KinigeApiError("chat_not_found");
      }
      this.agentChats = [...new Set(chatIds)];
      this.emitSettingsChanged();
    });
  }

  listAgentLog(before?: number): Promise<AgentLogPage> {
    return this.run("listAgentLog", String(before ?? ""), () => {
      const older = this.logRows.filter((row) => before === undefined || row.id < before);
      const rows = older.slice(0, MOCK_LOG_PAGE);
      return { rows, older: older.length > MOCK_LOG_PAGE ? (rows[rows.length - 1]?.id ?? null) : null };
    });
  }

  listAgentLogEntries(logId: number): Promise<AgentLogEntry[]> {
    return this.run("listAgentLogEntries", String(logId), () => this.logEntries.get(logId) ?? []);
  }
}
