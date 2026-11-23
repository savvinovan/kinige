import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import type { Settings } from "../../api/generated/Settings";
import { stubCanvas } from "../forest/testCanvas";
import { fireEvent, render, screen } from "../../test/render";
import { MainWindow } from "./MainWindow";

// "Show forest" brings back the view the forest had when it was hidden (KINIG-150): the strip or the board.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const MESSAGES = [
  mockMessage({ id: "a3", chatId: INBOX_ID, bodyMd: "Today in the inbox", journalDate: "2026-10-21", createdAt: Date.parse("2026-10-21T09:00:00Z") }),
];

beforeEach(() => stubCanvas());
afterEach(() => vi.restoreAllMocks());

function setup(settings: Partial<Settings> = {}) {
  const api = new MockApi({ now: NOW, language: "en", messages: MESSAGES });
  render(<MainWindow api={api} initialSettings={{ ...DEFAULT_SETTINGS, ...settings }} />, { language: "en" });
  return api;
}

const toggle = () => screen.getByRole("button", { name: "Forest" });
const board = () => screen.findByRole("application", { name: "Forest of 2026" });
const strip = () => screen.findByRole("grid", { name: "Days of 2026" });
const menuItem = async (name: string) => {
  fireEvent.click(screen.getByRole("button", { name: /^Profile/ }));
  return screen.findByRole("menuitem", { name });
};

describe("showing the forest again", () => {
  it("brings the board back after the board was hidden, from the top bar button", async () => {
    const api = setup();
    await strip();
    fireEvent.click(screen.getByRole("button", { name: "Expand forest" }));
    await board();
    fireEvent.click(toggle());
    expect(screen.queryByRole("region", { name: "Forest" })).toBeNull();
    fireEvent.click(toggle());
    expect(await board()).toBeInTheDocument();
    expect(screen.queryByRole("grid")).toBeNull();
    expect(api.calls.filter((c) => c.startsWith("setForestView"))).toEqual([
      "setForestView expanded forest",
      "setForestView hidden forest",
      "setForestView expanded forest",
    ]);
  });

  it("brings the board back from the profile menu as well, and the calendar mode stays", async () => {
    setup({ forestView: "expanded", forestMode: "calendar" });
    await screen.findByRole("group", { name: "Calendar of 2026" });
    fireEvent.click(await menuItem("Hide forest"));
    expect(screen.queryByRole("region", { name: "Forest" })).toBeNull();
    fireEvent.click(await menuItem("Show forest"));
    expect(await screen.findByRole("group", { name: "Calendar of 2026" })).toBeInTheDocument();
  });

  it("brings the strip back after the strip was hidden", async () => {
    setup();
    await strip();
    fireEvent.click(toggle());
    fireEvent.click(toggle());
    expect(await strip()).toBeInTheDocument();
  });

  it("starts hidden and shows the remembered view: the board after a restart, the strip when there is none", async () => {
    setup({ forestView: "hidden", forestShownView: "expanded" });
    await screen.findByText("Today in the inbox");
    fireEvent.click(toggle());
    expect(await board()).toBeInTheDocument();
  });

  it("an older database that says hidden and remembers nothing shows the strip", async () => {
    setup({ forestView: "hidden" });
    await screen.findByText("Today in the inbox");
    fireEvent.click(toggle());
    expect(await strip()).toBeInTheDocument();
  });

  it("the mock repeats Rust: hiding keeps the remembered view, a shown view replaces it", async () => {
    const api = new MockApi({ now: NOW, language: "en", messages: MESSAGES });
    await api.setForestView("expanded", "forest");
    await api.setForestView("hidden", "forest");
    expect(await api.getSettings()).toMatchObject({ forestView: "hidden", forestShownView: "expanded" });
    await api.setForestView("strip", "forest");
    await api.setForestView("hidden", "forest");
    expect(await api.getSettings()).toMatchObject({ forestView: "hidden", forestShownView: "strip" });
  });
});
