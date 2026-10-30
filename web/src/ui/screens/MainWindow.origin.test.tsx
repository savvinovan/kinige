import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import type { Settings } from "../../api/generated/Settings";
import { render, screen } from "../../test/render";
import { DEFAULT_SHORTCUTS } from "../settings/demoShortcuts";
import { MainWindow } from "./MainWindow";

const SETTINGS: Settings = {
  ...DEFAULT_SETTINGS,
  mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
  shortcuts: DEFAULT_SHORTCUTS,
};

describe("Пометка в ленте (слайс 4a)", () => {
  it("запись агента: «добавлено агентом» в строке времени; запись пользователя — без неё", async () => {
    const api = new MockApi({
      now: () => new Date(Date.UTC(2026, 9, 28, 12, 0)),
      messages: [
        mockMessage({ id: "u", bodyMd: "Моя запись", createdAt: Date.UTC(2026, 9, 28, 9, 0), journalDate: "2026-10-28" }),
        mockMessage({ id: "a", bodyMd: "Запись агента", createdAt: Date.UTC(2026, 9, 28, 9, 5), journalDate: "2026-10-28", origin: "agent" }),
      ],
    });
    render(<MainWindow api={api} initialSettings={SETTINGS} />);
    const agent = (await screen.findByText("Запись агента")).closest("article");
    const mine = (await screen.findByText("Моя запись")).closest("article");
    expect(agent).toHaveTextContent("добавлено агентом");
    expect(agent?.querySelector(".message__origin")).not.toBeNull();
    expect(mine).not.toHaveTextContent("добавлено агентом");
  });
});
