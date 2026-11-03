import { KinigeApiError } from "./KinigeApi";
import { MockApiSlice4 } from "./MockApiSlice4";
import type { AgentAccess } from "./generated/AgentAccess";
import type { AgentLogEntry } from "./generated/AgentLogEntry";
import type { AgentLogPage } from "./generated/AgentLogPage";
import type { AgentLogRow } from "./generated/AgentLogRow";
import type { AgentMode } from "./generated/AgentMode";
import type { AgentSocket } from "./generated/AgentSocket";

/** Log rows per page (like `LOG_PAGE` in Rust). */
export const MOCK_LOG_PAGE = 30;

const BRIDGE = "/Applications/Kinige.app/Contents/MacOS/kinige-mcp";
const SOCKET = "/Users/me/Library/Application Support/me.savvinov.kinige/mcp/kinige.sock";

/** Socket state, the snippet for Claude Desktop and the command for Claude Code, as Rust sends them. */
export const MOCK_AGENT_SOCKET: AgentSocket = {
  listening: true,
  error: null,
  bridgePath: BRIDGE,
  socketPath: SOCKET,
  desktopConfig: JSON.stringify({ mcpServers: { kinige: { command: BRIDGE, args: ["--socket", SOCKET] } } }, null, 2),
  codeCommand: `claude mcp add --scope user kinige -- ${BRIDGE} --socket '${SOCKET}'`,
};

/**
 * AI agent access in `MockApi` (slice 4a): the flag, the mode, the visible chats (`ai_excluded` ones are not accepted, as in
 * Rust), the call log by pages and the entries of a row. A test fills the log and the row entries through `addAgentLog`.
 */
export abstract class MockApiSlice4a extends MockApiSlice4 {
  /** The "settings changed" event (a window edit): windows listen to it, implemented in `MockApiSlice4b`. */
  abstract emitSettingsChanged(): void;

  agentEnabled = false;
  agentMode: AgentMode = "read";
  /** Ids of the chats marked "visible to the agent". */
  agentChats: string[] = [];
  /** Chats with `ai_excluded` (the chat mock has no such flag). */
  readonly agentExcluded = new Set<string>();
  agentSocket: AgentSocket = MOCK_AGENT_SOCKET;
  private logRows: AgentLogRow[] = [];
  private readonly logEntries = new Map<number, AgentLogEntry[]>();

  /** A new log row (older rows come earlier in the call order). Returns its `id`. */
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
