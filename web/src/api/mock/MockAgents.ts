import { KinigeApiError } from "../KinigeApi";
import { LIMITS } from "../mockRules";
import type { AgentAccess } from "../generated/AgentAccess";
import type { AgentLogEntry } from "../generated/AgentLogEntry";
import type { AgentLogPage } from "../generated/AgentLogPage";
import type { AgentLogRow } from "../generated/AgentLogRow";
import type { AgentMode } from "../generated/AgentMode";
import type { AgentSocket } from "../generated/AgentSocket";
import type { MockState } from "./MockState";

/** Log rows per page (`LOG_PAGE` in Rust, pinned by `fixtures/api-contract/constants.json`). */
export const MOCK_LOG_PAGE = LIMITS.agentLogPage;

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
 * AI agent access in `MockApi` (slice 4a): the flag, the mode, the visible chats (`ai_excluded` ones are not accepted, as in Rust), the
 * call log by pages and the entries of a row. A test fills the log and the row entries through `addAgentLog`.
 */
export class MockAgents {
  enabled = false;
  mode: AgentMode = "read";
  /** Ids of the chats marked "visible to the agent". */
  chats: string[] = [];
  /** Chats with `ai_excluded` (the chat mock has no such flag). */
  readonly excluded = new Set<string>();
  socket: AgentSocket = MOCK_AGENT_SOCKET;
  private logRows: AgentLogRow[] = [];
  private readonly logEntries = new Map<number, AgentLogEntry[]>();

  constructor(private readonly s: MockState) {}

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
    return this.s.run("getAgentAccess", "", () => ({
      enabled: this.enabled,
      mode: this.mode,
      chats: this.s.chats.map((chat) => {
        const aiExcluded = this.excluded.has(chat.id);
        return {
          id: chat.id,
          title: chat.title,
          icon: chat.icon,
          isInbox: chat.isInbox,
          count: chat.count,
          aiExcluded,
          visible: !aiExcluded && this.chats.includes(chat.id),
        };
      }),
      socket: this.socket,
    }));
  }

  setAgentEnabled(enabled: boolean): Promise<void> {
    return this.s.run("setAgentEnabled", String(enabled), () => {
      this.enabled = enabled;
      this.s.emitSettingsChanged();
    });
  }

  setAgentMode(mode: AgentMode): Promise<void> {
    return this.s.run("setAgentMode", mode, () => {
      this.mode = mode;
      this.s.emitSettingsChanged();
    });
  }

  setAgentChats(chatIds: readonly string[]): Promise<void> {
    return this.s.run("setAgentChats", chatIds.join(","), () => {
      for (const id of chatIds) {
        if (this.excluded.has(id) || !this.s.chats.some((chat) => chat.id === id)) throw new KinigeApiError("chat_not_found");
      }
      this.chats = [...new Set(chatIds)];
      this.s.emitSettingsChanged();
    });
  }

  listAgentLog(before?: number): Promise<AgentLogPage> {
    return this.s.run("listAgentLog", String(before ?? ""), () => {
      const older = this.logRows.filter((row) => before === undefined || row.id < before);
      const rows = older.slice(0, MOCK_LOG_PAGE);
      return { rows, older: older.length > MOCK_LOG_PAGE ? (rows[rows.length - 1]?.id ?? null) : null };
    });
  }

  listAgentLogEntries(logId: number): Promise<AgentLogEntry[]> {
    return this.s.run("listAgentLogEntries", String(logId), () => this.logEntries.get(logId) ?? []);
  }
}
