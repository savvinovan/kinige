import { describe, expect, it, vi } from "vitest";
import type { AgentAccess } from "../../../api/generated/AgentAccess";
import { MOCK_AGENT_SOCKET } from "../../../api/MockApiSlice4a";
import { render, screen, within } from "../../../test/render";
import { AccessPage } from "./AccessPage";

const access = (patch: Partial<AgentAccess> = {}): AgentAccess => ({ enabled: false, mode: "read", chats: [], socket: MOCK_AGENT_SOCKET, ...patch });
const unsupported = access({ enabled: true, socket: { ...MOCK_AGENT_SOCKET, listening: false, error: "unsupported", bridgePath: null, socketPath: null, desktopConfig: null, codeCommand: null } });

function page(value: AgentAccess, language: "ru" | "en") {
  render(<AccessPage access={value} onEnabledChange={vi.fn()} onModeChange={vi.fn()} onOpen={vi.fn()} changedAt={null} />, { language });
  return screen.getByRole("region", { name: /Доступ ИИ-агентов|AI agent access/ });
}

describe("AI agent access: a platform without an MCP server (Windows, slice 10a)", () => {
  it("one quiet line instead of the switch, the mode and the lists, in both languages", () => {
    const ru = page(unsupported, "ru");
    expect(within(ru).getByText("Пока недоступно в Windows.")).toBeInTheDocument();
    expect(within(ru).queryByRole("checkbox")).toBeNull();
    expect(within(ru).queryByRole("radio")).toBeNull();
    expect(within(ru).queryByRole("button")).toBeNull();
    expect(within(ru).queryByRole("alert")).toBeNull();
  });

  it("the English text", () => {
    const en = page(unsupported, "en");
    expect(within(en).getByRole("heading", { level: 2, name: "AI agent access" })).toBeInTheDocument();
    expect(within(en).getByText("Not available on Windows yet.")).toBeInTheDocument();
    expect(within(en).queryByRole("checkbox")).toBeNull();
  });

  it("an ordinary socket state leaves the page as it was: the switch is there", () => {
    const ru = page(access(), "ru");
    expect(within(ru).getByRole("checkbox", { name: "Разрешить агентам доступ к журналу" })).toBeInTheDocument();
    expect(within(ru).queryByText("Пока недоступно в Windows.")).toBeNull();
  });
});
