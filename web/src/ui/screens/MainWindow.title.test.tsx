import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import { INBOX_ID } from "../../api/KinigeApi";
import { stubCanvas } from "../forest/testCanvas";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// The open chat's name at the left of the top bar (KINIG-150): the sidebar's `displayName`, Inbox translated, following a switch and a rename,
// and no bar in the trash or in search results.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const LONG = "Очень длинное название чата ".repeat(8).slice(0, 128);

beforeEach(() => stubCanvas());
afterEach(() => vi.restoreAllMocks());

function setup(language: "ru" | "en" = "ru") {
  const api = new MockApi({
    now: NOW,
    language,
    chats: [
      { id: "books", title: "Книги", icon: "📚" },
      { id: "long", title: LONG, icon: "📝" },
    ],
    messages: [mockMessage({ id: "a1", chatId: INBOX_ID, bodyMd: "Today in the inbox", journalDate: "2026-10-21", createdAt: Date.parse("2026-10-21T09:00:00Z") })],
  });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language });
  return api;
}

const title = () => screen.getByRole("heading", { level: 1 });
const bar = () => title().parentElement!;

describe("the chat name in the top bar", () => {
  it("says Inbox translated, at the left end, before the forest switch", async () => {
    setup("en");
    await screen.findByText("Today in the inbox");
    expect(title()).toHaveTextContent("Inbox");
    expect(bar()).toHaveClass("main-window__topbar");
    expect(title().compareDocumentPosition(screen.getByRole("button", { name: "Forest" })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("says the name in Russian too, and follows a switch to another chat", async () => {
    setup("ru");
    await screen.findByText("Today in the inbox");
    expect(title()).toHaveTextContent("Входящие");
    fireEvent.click(await screen.findByRole("button", { name: /^Книги/ }));
    await waitFor(() => expect(title()).toHaveTextContent("Книги"));
    fireEvent.click(screen.getByRole("button", { name: /^Входящие/ }));
    await waitFor(() => expect(title()).toHaveTextContent("Входящие"));
  });

  it("follows a rename at once", async () => {
    setup("ru");
    fireEvent.click(await screen.findByRole("button", { name: /^Книги/ }));
    await waitFor(() => expect(title()).toHaveTextContent("Книги"));
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Книги/ }));
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Переименовать" }));
    const dialog = screen.getByRole("dialog", { name: "Переименовать чат" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Название" }), { target: { value: "Читаю" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Сохранить" }));
    await waitFor(() => expect(title()).toHaveTextContent("Читаю"));
  });

  it("keeps a long name whole for the screen reader and the tooltip; the line is cut by the style, the switch is still one button", async () => {
    setup("ru");
    fireEvent.click(await screen.findByRole("button", { name: /^Очень/ }));
    await waitFor(() => expect(title()).toHaveTextContent(LONG));
    expect(title()).toHaveAttribute("title", LONG);
    expect(screen.getAllByRole("button", { name: "Лес" })).toHaveLength(1);
  });

  it("is not above the trash or the search results", async () => {
    setup("en");
    await screen.findByText("Today in the inbox");
    fireEvent.click(screen.getByRole("button", { name: /^Trash/ }));
    await waitFor(() => expect(screen.queryByRole("heading", { level: 1 })).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: /^Inbox/ }));
    await waitFor(() => expect(screen.getByRole("heading", { level: 1 })).toBeInTheDocument());
    fireEvent.change(screen.getByRole("combobox", { name: "Search" }), { target: { value: "inbox" } });
    await screen.findByRole("region", { name: "Search results" });
    expect(screen.queryByRole("heading", { level: 1 })).toBeNull();
  });
});
