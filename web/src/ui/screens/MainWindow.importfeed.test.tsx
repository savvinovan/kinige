import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import type { ImportPreview } from "../../api/generated/ImportPreview";
import { FakeObserver, edges, showEdge } from "../../test/fakeObserver";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Лента после импорта: записи легли в середину истории (в базе уже были свежие), открытый Inbox должен перечитаться с нуля,
// иначе курсор «старше» остаётся прежним и страницы между ними никогда не загрузятся.
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

/** Верхний край показывается, пока он есть: каждая подгрузка добавляет записи, в конце истории края нет. */
async function loadAllOlder(container: HTMLElement) {
  for (let step = 0; step < 6 && edges(container).length > 0; step++) {
    const before = container.querySelectorAll("article").length;
    showEdge(container, "older");
    await waitFor(() => expect(container.querySelectorAll("article").length).toBeGreaterThan(before));
  }
}

/** Старые записи, которые «записал» импорт (мок сам записей не создаёт). */
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
    // До импорта все три записи на одной странице: курсора «старше» нет.
    expect(edges(container)).toHaveLength(0);
    addImported(api, 0, 450);
    await runImport();
    fireEvent.click(screen.getByRole("button", { name: "Готово" }));
    await screen.findByText("старая 449");
    // Край сверху появился, подгрузка идёт до самой старой записи.
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
    // Первый приём: самые свежие из старых записей.
    addImported(api, 250, 200);
    await runImport();
    fireEvent.click(screen.getByRole("button", { name: "Готово" }));
    await screen.findByText("старая 449");
    // Второй приём (окно закрыто, приложение не перезапускали): ещё более ранние записи.
    addImported(api, 0, 250);
    await runImport();
    fireEvent.click(screen.getByRole("button", { name: "Готово" }));
    await waitFor(() => expect(edges(container)).toHaveLength(1));
    await loadAllOlder(container);
    await screen.findByText("старая 0");
    // Ровно 451 запись: ни одна не потеряна между первым и вторым приёмом.
    await waitFor(() => expect(container.querySelectorAll("article")).toHaveLength(451));
    for (const i of [0, 100, 249, 250, 300, 449]) expect(screen.getByText(`старая ${i}`)).toBeInTheDocument();
  });
});
