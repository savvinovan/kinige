import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import type { ImportPreview } from "../../api/generated/ImportPreview";
import { FakeObserver, edges, showEdge } from "../../test/fakeObserver";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// The feed after an import: entries landed in the middle of the history (fresh ones were already in the database), the open Inbox must be reread from scratch,
// otherwise the "older" cursor stays the same and the pages between them are never loaded.
vi.setConfig({ testTimeout: 30_000 });

const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const DAY = 24 * 3_600_000;
const OLD_START = Date.UTC(2019, 2, 8, 12);
const day = (at: number) => new Date(at - 4 * 3_600_000).toISOString().slice(0, 10);

const PREVIEW: ImportPreview = {
  chats: [{ id: "1000001", name: null, kind: "saved", total: 450, existing: 0 }],
  skippedChats: 0,
};

beforeEach(() => {
  FakeObserver.all = [];
  vi.stubGlobal("IntersectionObserver", FakeObserver);
});
afterEach(() => vi.unstubAllGlobals());

async function runImport() {
  fireEvent.click(await screen.findByRole("button", { name: /^Профиль/ }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Импорт из Telegram…" }));
  const dialog = await screen.findByRole("dialog", { name: "Импорт из Telegram" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Выбрать result.json…" }));
  await within(dialog).findByRole("list", { name: "Чаты для импорта" });
  fireEvent.click(within(dialog).getByRole("button", { name: "Импортировать" }));
  await within(screen.getByRole("dialog")).findByRole("heading", { name: "Импорт завершён" });
}

/** The top edge is shown while it exists: each load adds entries, at the end of the history there is no edge. */
async function loadAllOlder(container: HTMLElement) {
  for (let step = 0; step < 6 && edges(container).length > 0; step++) {
    const before = container.querySelectorAll("article").length;
    await showEdge(container, "older");
    await waitFor(() => expect(container.querySelectorAll("article").length).toBeGreaterThan(before));
  }
}

/** Old entries that the import "wrote" (the mock does not create entries itself). */
function addImported(api: MockApi, from: number, count: number) {
  for (let i = from; i < from + count; i++) {
    const createdAt = OLD_START + i * DAY;
    api.addExternalMessage(mockMessage({ id: `old${i}`, bodyMd: `старая ${i}`, createdAt, journalDate: day(createdAt) }));
  }
}

describe("Лента после импорта", () => {
  it("«Готово» → подгрузка вверх идёт страница за страницей до самой старой записи, без дыр", async () => {
    const api = new MockApi({
      now: NOW,
      messages: [0, 1, 2].map((i) =>
        mockMessage({ id: `new${i}`, bodyMd: `свежая ${i}`, createdAt: Date.UTC(2026, 9, 21, 1 + i), journalDate: "2026-10-21" }),
      ),
    });
    api.importFile = PREVIEW;
    const { container } = render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
    await screen.findByText("свежая 2");
    // Before the import all three entries are on one page: there is no "older" cursor.
    expect(edges(container)).toHaveLength(0);
    addImported(api, 0, 450);
    await runImport();
    fireEvent.click(screen.getByRole("button", { name: "Готово" }));
    await screen.findByText("старая 449");
    // The top edge appeared, loading goes up to the oldest entry.
    await waitFor(() => expect(edges(container)).toHaveLength(1));
    await loadAllOlder(container);
    await screen.findByText("старая 0");
    await waitFor(() => expect(edges(container)).toHaveLength(0));
    expect(container.querySelectorAll("article")).toHaveLength(453);
    expect(screen.getByText("свежая 2")).toBeInTheDocument();
  });

  it("импорт в несколько приёмов без перезапуска: между страницами нет дыры", async () => {
    const api = new MockApi({
      now: NOW,
      messages: [mockMessage({ id: "new0", bodyMd: "свежая 0", createdAt: Date.UTC(2026, 9, 21, 1), journalDate: "2026-10-21" })],
    });
    api.importFile = PREVIEW;
    const { container } = render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
    await screen.findByText("свежая 0");
    // The first intake: the freshest of the old entries.
    addImported(api, 250, 200);
    await runImport();
    fireEvent.click(screen.getByRole("button", { name: "Готово" }));
    await screen.findByText("старая 449");
    // The second intake (the window is closed, the app was not restarted): even earlier entries.
    addImported(api, 0, 250);
    await runImport();
    fireEvent.click(screen.getByRole("button", { name: "Готово" }));
    await waitFor(() => expect(edges(container)).toHaveLength(1));
    await loadAllOlder(container);
    await screen.findByText("старая 0");
    // Exactly 451 entries: none lost between the first and second intake.
    await waitFor(() => expect(container.querySelectorAll("article")).toHaveLength(451));
    for (const i of [0, 100, 249, 250, 300, 449]) expect(screen.getByText(`старая ${i}`)).toBeInTheDocument();
  });
});
