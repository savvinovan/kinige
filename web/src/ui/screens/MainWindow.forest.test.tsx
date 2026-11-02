import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, type MockApiOptions, mockMessage } from "../../api/MockApi";
import type { Settings } from "../../api/generated/Settings";
import { stubCanvas } from "../forest/testCanvas";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { rect } from "../../test/rect";
import { MainWindow } from "./MainWindow";

// The forest above the feed on the real `MainWindow` and `MockApi` (slice 6). "Now" is Wednesday 21 October 2026.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const at = (date: string, hour = 9) => Date.parse(`${date}T${String(hour).padStart(2, "0")}:00:00Z`);
const entry = (id: string, date: string, body: string, chatId = INBOX_ID, hour = 9) =>
  mockMessage({ id, chatId, bodyMd: body, journalDate: date, createdAt: at(date, hour) });

const MESSAGES = [
  entry("n1", "2025-12-30", "A note from the end of last year"),
  entry("a1", "2026-10-14", "Inbox note of the fourteenth"),
  entry("b1", "2026-10-15", "Books note of the fifteenth", "books"),
  entry("b2", "2026-10-15", "Another books note", "books", 10),
  entry("a2", "2026-10-15", "One inbox note of the fifteenth", INBOX_ID, 8),
  entry("a3", "2026-10-21", "Today in the inbox"),
];

beforeEach(() => stubCanvas());
afterEach(() => vi.restoreAllMocks());

function setup(options: MockApiOptions = {}, settings: Partial<Settings> = {}) {
  const api = new MockApi({ now: NOW, language: "en", chats: [{ id: "books", title: "Books", icon: "📚" }], messages: MESSAGES, ...options });
  const view = render(<MainWindow api={api} initialSettings={{ ...DEFAULT_SETTINGS, ...settings }} />, { language: "en" });
  return { api, ...view };
}

const pane = () => screen.queryByRole("region", { name: "Forest" });
const cell = (date: string) => document.querySelector<HTMLElement>(`[data-date="${date}"]`)!;
const forestCalls = (api: MockApi) => api.calls.filter((c) => c.startsWith("getForestYear"));
/** jsdom has no layout: puts the first element of `topDate` at the top edge of the feed (earlier ones above, later below) and scrolls. */
function placeFeed(topDate: string) {
  const scroller = screen.getByRole("region", { name: "Entries" });
  scroller.getBoundingClientRect = () => rect(0, 100, 600, 400);
  const nodes = [...scroller.querySelectorAll<HTMLElement>("[data-journal-date]")];
  const first = nodes.findIndex((node) => node.dataset.journalDate === topDate);
  nodes.forEach((node, i) => {
    const top = 100 + (i - first) * 60;
    node.getBoundingClientRect = () => rect(0, top, 600, 50);
  });
  fireEvent.wheel(scroller);
  fireEvent.scroll(scroller);
}
const menuItem = async (name: string) => {
  fireEvent.click(screen.getByRole("button", { name: /^Profile/ }));
  return screen.findByRole("menuitem", { name });
};

describe("the forest above the feed", () => {
  it("is a strip above the feed by default, in the open chat", async () => {
    setup();
    await screen.findByText("Today in the inbox");
    expect(await screen.findByRole("grid", { name: "Days of 2026" })).toBeInTheDocument();
    expect(pane()).toBeInTheDocument();
    const feed = screen.getByRole("region", { name: "Entries" });
    expect(pane()!.compareDocumentPosition(feed) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    fireEvent.click(await screen.findByRole("button", { name: /^Books/ }));
    await screen.findByText("Books note of the fifteenth");
    expect(pane()).toBeInTheDocument();
  });

  it("is not shown in the trash or in search results", async () => {
    setup();
    await screen.findByText("Today in the inbox");
    fireEvent.click(screen.getByRole("button", { name: /^Trash/ }));
    await waitFor(() => expect(pane()).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: /^Inbox/ }));
    await waitFor(() => expect(pane()).toBeInTheDocument());
    fireEvent.change(screen.getByRole("combobox", { name: "Search" }), { target: { value: "note" } });
    await screen.findByRole("region", { name: "Search results" });
    expect(pane()).toBeNull();
  });

  it("loads the year once with the first weekday and the source of it, then reloads without the source", async () => {
    const { api } = setup();
    await screen.findByRole("grid", { name: "Days of 2026" });
    expect(forestCalls(api)[0]).toMatch(/^getForestYear 2026 (monday|sunday)$/);
    expect(api.weekdaySources).toHaveLength(1);
    fireEvent.change(screen.getByRole("textbox", { name: "New note" }), { target: { value: "one more" } });
    fireEvent.keyDown(screen.getByRole("textbox", { name: "New note" }), { key: "Enter" });
    await waitFor(() => expect(forestCalls(api).length).toBeGreaterThan(1));
    expect(api.weekdaySources).toHaveLength(1);
  });

});

describe("expanding, hiding and coming back", () => {
  it("expand shows the board, collapse the strip; both are remembered", async () => {
    const { api } = setup();
    await screen.findByRole("grid", { name: "Days of 2026" });
    fireEvent.click(screen.getByRole("button", { name: "Expand forest" }));
    expect(await screen.findByRole("application", { name: "Forest of 2026" })).toBeInTheDocument();
    expect(screen.queryByRole("grid")).toBeNull();
    expect(api.calls).toContain("setForestView expanded forest");
    fireEvent.click(screen.getByRole("button", { name: "Collapse forest" }));
    expect(await screen.findByRole("grid", { name: "Days of 2026" })).toBeInTheDocument();
    expect(api.calls.at(-1)).toBe("setForestView strip forest");
  });

  it("Forest / Calendar switches the same data and is remembered", async () => {
    const { api } = setup({}, { forestView: "expanded" });
    await screen.findByRole("application", { name: "Forest of 2026" });
    fireEvent.click(screen.getByRole("radio", { name: "Calendar" }));
    expect(await screen.findByRole("group", { name: "Calendar of 2026" })).toBeInTheDocument();
    expect(screen.queryByRole("application")).toBeNull();
    expect(api.calls.at(-1)).toBe("setForestView expanded calendar");
    // The season buttons belong to the board only.
    expect(screen.queryByRole("button", { name: "Winter" })).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "Forest" }));
    expect(await screen.findByRole("application", { name: "Forest of 2026" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Winter" })).toBeInTheDocument();
  });

  it("'Hide forest' on the strip removes the pane, and the profile menu brings the strip back", async () => {
    const { api } = setup();
    await screen.findByRole("grid", { name: "Days of 2026" });
    fireEvent.click(screen.getByRole("button", { name: "Hide forest" }));
    expect(pane()).toBeNull();
    expect(api.calls.at(-1)).toBe("setForestView hidden forest");
    const callsWhileHidden = forestCalls(api).length;
    fireEvent.click(await menuItem("Show forest"));
    expect(await screen.findByRole("grid", { name: "Days of 2026" })).toBeInTheDocument();
    expect(api.calls).toContain("setForestView strip forest");
    expect(forestCalls(api).length).toBeGreaterThan(callsWhileHidden);
  });

  it("hiding from the expanded header and showing again gives the strip, not the board", async () => {
    setup({}, { forestView: "expanded" });
    await screen.findByRole("application", { name: "Forest of 2026" });
    fireEvent.click(screen.getByRole("button", { name: "Hide forest" }));
    expect(pane()).toBeNull();
    fireEvent.click(await menuItem("Show forest"));
    expect(await screen.findByRole("grid", { name: "Days of 2026" })).toBeInTheDocument();
  });

  it("the profile menu says 'Hide forest' while it is shown", async () => {
    setup();
    await screen.findByRole("grid", { name: "Days of 2026" });
    expect(await menuItem("Hide forest")).toBeInTheDocument();
  });

  it("starts as saved: hidden asks Rust for no table at all", async () => {
    const { api } = setup({}, { forestView: "hidden", forestMode: "calendar" });
    await screen.findByText("Today in the inbox");
    expect(pane()).toBeNull();
    expect(forestCalls(api)).toEqual([]);
    fireEvent.click(await menuItem("Show forest"));
    await screen.findByRole("grid", { name: "Days of 2026" });
    expect(forestCalls(api)).toHaveLength(1);
  });

  it("starts expanded in calendar mode when that was saved", async () => {
    setup({}, { forestView: "expanded", forestMode: "calendar" });
    expect(await screen.findByRole("group", { name: "Calendar of 2026" })).toBeInTheDocument();
  });
});

describe("a click on a day opens the feed there", () => {
  it("opens the first entry of the day in the open chat and highlights it like a jump from search", async () => {
    const { api } = setup();
    await screen.findByRole("grid", { name: "Days of 2026" });
    await waitFor(() => expect(cell("2026-10-15")).not.toBeNull());
    fireEvent.click(cell("2026-10-15"));
    // The open chat (Inbox) has an entry that day: its first one.
    const plate = (await screen.findByText("One inbox note of the fifteenth")).closest("article") as HTMLElement;
    await waitFor(() => expect(plate).toHaveClass("message--highlighted"));
    expect(api.calls).toContain(`getForestDayTarget 2026-10-15 ${INBOX_ID}`);
    // The outline stays on the clicked day.
    expect(cell("2026-10-15")).toHaveAttribute("aria-selected", "true");
  });

  it("goes to the chat with the most entries that day when the open chat has none", async () => {
    setup();
    await screen.findByText("Today in the inbox");
    fireEvent.click(screen.getByRole("button", { name: /^Books/ }));
    await screen.findByText("Books note of the fifteenth");
    fireEvent.click(cell("2026-10-14"));
    // Books has nothing on the 14th, the Inbox does: the chat switches.
    const plate = (await screen.findByText("Inbox note of the fourteenth")).closest("article") as HTMLElement;
    await waitFor(() => expect(plate).toHaveClass("message--highlighted"));
    expect(screen.getByRole("button", { name: /^Inbox/ })).toHaveAttribute("aria-current", "true");
  });

  it("a day without entries opens nothing", async () => {
    setup();
    await screen.findByRole("grid", { name: "Days of 2026" });
    fireEvent.click(cell("2026-10-10"));
    await waitFor(() => expect(document.querySelectorAll(".message--highlighted")).toHaveLength(0));
    expect(screen.getByText("Today in the inbox")).toBeInTheDocument();
  });

  it("works from the calendar the same way", async () => {
    setup({}, { forestView: "expanded", forestMode: "calendar" });
    const group = await screen.findByRole("group", { name: "Calendar of 2026" });
    fireEvent.click(within(group).getByRole("button", { name: /^October 15, / }));
    const plate = (await screen.findByText("One inbox note of the fifteenth")).closest("article") as HTMLElement;
    await waitFor(() => expect(plate).toHaveClass("message--highlighted"));
  });
});

describe("the forest follows the journal", () => {
  it("a new note from the capture window reloads the table", async () => {
    const { api } = setup();
    await screen.findByRole("grid", { name: "Days of 2026" });
    await waitFor(() => expect(cell("2026-10-21")).toHaveAttribute("data-stage", "0"));
    const before = forestCalls(api).length;
    act(() => {
      api.addExternalMessage(entry("ext", "2026-10-21", "x".repeat(400), INBOX_ID, 11));
      api.emitEntriesChanged();
    });
    await waitFor(() => expect(forestCalls(api).length).toBeGreaterThan(before));
    await waitFor(() => expect(Number(cell("2026-10-21").dataset.stage)).toBeGreaterThan(0));
  });

  it("deleting an entry reloads it too", async () => {
    const { api } = setup();
    await screen.findByText("Today in the inbox");
    const before = forestCalls(api).length;
    const plate = (await screen.findByText("Today in the inbox")).closest("article") as HTMLElement;
    fireEvent.contextMenu(plate);
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Delete" }));
    await waitFor(() => expect(forestCalls(api).length).toBeGreaterThan(before));
  });

  it("the outlined day follows the day at the top of the feed inside a year, without reloading the year", async () => {
    const { api } = setup();
    await screen.findByText("Today in the inbox");
    await screen.findByRole("grid", { name: "Days of 2026" });
    const before = forestCalls(api).length;
    placeFeed("2026-10-14");
    await waitFor(() => expect(cell("2026-10-14")).toHaveAttribute("aria-selected", "true"));
    placeFeed("2026-10-15");
    await waitFor(() => expect(cell("2026-10-15")).toHaveAttribute("aria-selected", "true"));
    expect(cell("2026-10-14")).toHaveAttribute("aria-selected", "false");
    expect(forestCalls(api)).toHaveLength(before);
  });

  it("when the feed crosses into the previous year, the board and the strip switch to it", async () => {
    const { api } = setup();
    await screen.findByText("A note from the end of last year");
    await screen.findByRole("grid", { name: "Days of 2026" });
    placeFeed("2026-10-14");
    await waitFor(() => expect(cell("2026-10-14")).toHaveAttribute("aria-selected", "true"));
    placeFeed("2025-12-30");
    await waitFor(() => expect(forestCalls(api).at(-1)).toMatch(/^getForestYear 2025 /));
    expect(await screen.findByRole("grid", { name: "Days of 2025" })).toBeInTheDocument();
    await waitFor(() => expect(cell("2025-12-30")).toHaveAttribute("aria-selected", "true"));
  });

  it("scrolling the feed back to this year brings this year back, and picking a year by hand is not undone by scrolling in it", async () => {
    const { api } = setup({}, { forestView: "expanded" });
    await screen.findByRole("application", { name: "Forest of 2026" });
    placeFeed("2026-10-14");
    // The feed's first report has arrived (the board is not told which cell, only the pane is).
    await waitFor(() => expect(forestCalls(api).length).toBeGreaterThan(0));
    await new Promise((resolve) => setTimeout(resolve, 50));
    fireEvent.click(screen.getByRole("button", { name: "Previous year" }));
    expect(await screen.findByRole("application", { name: "Forest of 2025" })).toBeInTheDocument();
    placeFeed("2026-10-15");
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.getByRole("application", { name: "Forest of 2025" })).toBeInTheDocument();
    expect(forestCalls(api).at(-1)).toMatch(/^getForestYear 2025 /);
  });

  it("'‹ year ›' goes back to the oldest year and not past this one", async () => {
    setup({}, { forestView: "expanded" });
    await screen.findByRole("application", { name: "Forest of 2026" });
    expect(screen.getByRole("button", { name: "Next year" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Previous year" }));
    expect(await screen.findByRole("application", { name: "Forest of 2025" })).toBeInTheDocument();
    // The oldest entry is from 2025.
    expect(screen.getByRole("button", { name: "Previous year" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next year" })).toBeEnabled();
  });

  it("a failed load shows a quiet note and keeps the controls", async () => {
    const { api } = setup();
    api.failNext("getForestYear", "store_read_failed");
    expect(await screen.findByText("The forest could not be loaded.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Expand forest" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hide forest" })).toBeInTheDocument();
    // The feed is not affected.
    expect(await screen.findByText("Today in the inbox")).toBeInTheDocument();
  });
});
