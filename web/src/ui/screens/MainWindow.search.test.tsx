import { afterEach, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, type MockApiOptions, mockMessage } from "../../api/MockApi";
import type { SearchPage } from "../../api/generated/SearchPage";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Поиск на настоящем `MainWindow` и `MockApi`: ввод с задержкой, фильтры, клавиатура, гонка ответов, пустой результат, ошибка,
// переход к записи. Стемминг в моке упрощён (подстрока): словоформы проверяют тесты Rust.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const DAY = Date.UTC(2026, 9, 21, 5, 0);

const MESSAGES = [
  mockMessage({ id: "a", bodyMd: "Читаю книгу про море", createdAt: DAY, journalDate: "2026-10-21" }),
  mockMessage({ id: "b", chatId: "books", bodyMd: "Книга о Раст", createdAt: DAY - 1000, journalDate: "2026-10-21", favoriteAt: 5 }),
  mockMessage({ id: "c", chatId: "books", bodyMd: "Старая книга", createdAt: DAY - 86_400_000 * 40, journalDate: "2026-09-10", isTask: true }),
  mockMessage({ id: "d", bodyMd: "Молоко и хлеб", createdAt: DAY - 2000, journalDate: "2026-10-21" }),
];

afterEach(() => vi.restoreAllMocks());

function setup(options: MockApiOptions = {}, language: "ru" | "en" = "ru") {
  const api = new MockApi({
    now: NOW,
    language,
    chats: [{ id: "books", title: "Книги", icon: "📚" }],
    messages: MESSAGES,
    ...options,
  });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language });
  return api;
}

const field = () => screen.getByRole("combobox", { name: "Поиск" }) as HTMLInputElement;
const type = (value: string) => fireEvent.change(field(), { target: { value } });
const results = () => screen.getByRole("region", { name: "Результаты поиска" });
const options = () => within(results()).getAllByRole("option");
/** Строки записей без строки «Чаты» (чат «Книги» совпадает с запросом «книг» по названию). */
const hits = () => options().filter((row) => !row.classList.contains("search-chat"));

describe("Поиск: ввод и результаты на месте ленты", () => {
  it("пока в поле пусто, лента как была; текст — результаты на её месте с задержкой, поле ввода сообщения скрыто", async () => {
    const api = setup();
    await screen.findByText("Читаю книгу про море");
    expect(screen.queryByRole("region", { name: "Результаты поиска" })).toBeNull();
    type("книг");
    // Запрос уходит не сразу, а после паузы в наборе.
    expect(api.calls.some((c) => c.startsWith("searchMessages"))).toBe(false);
    expect(await within(await screen.findByRole("region", { name: "Результаты поиска" })).findAllByRole("option")).toHaveLength(4);
    expect(api.calls.filter((c) => c.startsWith("searchMessages"))).toEqual(["searchMessages книг"]);
    expect(screen.queryByRole("textbox", { name: "Новая заметка" })).toBeNull();
  });

  it("строка результата: эмодзи и имя чата, день, фрагмент с подсвеченными словами; чат, чьё имя совпало, — отдельной строкой «Чаты»", async () => {
    setup();
    await screen.findByText("Читаю книгу про море");
    type("книг");
    await waitFor(() => expect(options().length).toBeGreaterThan(0));
    const rows = options();
    // Новые сверху; первая — из Входящих.
    expect(rows[0]).toHaveTextContent("Книги"); // строка «Чаты»: чат «Книги» совпал по названию
    const first = rows[1];
    expect(first).toHaveTextContent("Входящие");
    expect(first).toHaveTextContent("Сегодня");
    const marked = [...(first?.querySelectorAll("mark") ?? [])].map((m) => m.textContent);
    expect(marked).toEqual(["книгу"]);
    expect(within(results()).getByText("Чаты")).toBeInTheDocument();
  });

  it("пустой результат — одна тихая строка; пустой запрос — подсказка; ошибка — строка ошибки", async () => {
    const api = setup();
    await screen.findByText("Читаю книгу про море");
    type("zzz");
    expect(await screen.findByText("Ничего не найдено.")).toBeInTheDocument();
    type("  ");
    expect(screen.queryByRole("region", { name: "Результаты поиска" })).toBeNull();
    api.failNext("searchMessages", "store_read_failed");
    type("книг");
    expect(await within(await screen.findByRole("region", { name: "Результаты поиска" })).findByRole("alert")).toBeInTheDocument();
  });

  it("устаревший ответ отбрасывается: медленный ответ на «кн» не затирает «молоко»", async () => {
    const api = setup();
    await screen.findByText("Читаю книгу про море");
    const real = api.searchMessages.bind(api);
    let release: (() => void) | null = null;
    api.searchMessages = (query, opts) => {
      const answer = real(query, opts);
      if (query !== "кн") return answer;
      return new Promise<SearchPage>((resolve) => {
        release = () => void answer.then(resolve);
      });
    };
    type("кн");
    await waitFor(() => expect(release).not.toBeNull());
    type("молоко");
    await waitFor(() => expect(options()).toHaveLength(1));
    await act(async () => {
      (release as (() => void) | null)?.();
      await Promise.resolve();
    });
    expect(options()).toHaveLength(1);
    expect(options()[0]).toHaveTextContent("Молоко и хлеб");
  });
});

describe("Поиск: фильтры", () => {
  it("«Этот чат» ищет только в открытом чате; «Избранное» и «Задачи» сужают; все вместе", async () => {
    const api = setup();
    fireEvent.click(await screen.findByRole("button", { name: /^Книги/ }));
    await screen.findByText("Книга о Раст");
    type("книг");
    await waitFor(() => expect(hits()).toHaveLength(3));
    fireEvent.click(within(results()).getByRole("radio", { name: "Этот чат" }));
    await waitFor(() => expect(hits()).toHaveLength(2));
    expect(api.calls.at(-1)).toBe("searchMessages книг");
    fireEvent.click(within(results()).getByRole("checkbox", { name: "Избранное" }));
    await waitFor(() => expect(hits()).toHaveLength(1));
    expect(hits()[0]).toHaveTextContent("Книга о Раст");
    fireEvent.click(within(results()).getByRole("checkbox", { name: "Избранное" }));
    fireEvent.click(within(results()).getByRole("checkbox", { name: "Задачи" }));
    await waitFor(() => expect(hits()[0]).toHaveTextContent("Старая книга"));
    expect(hits()).toHaveLength(1);
  });

  it("«С даты» открывает календарь; выбранная дата сужает результаты и сбрасывается крестиком", async () => {
    setup();
    await screen.findByText("Читаю книгу про море");
    type("книг");
    await waitFor(() => expect(hits()).toHaveLength(3));
    fireEvent.click(within(results()).getByRole("button", { name: "С даты" }));
    const calendar = await screen.findByRole("dialog", { name: "Искать с даты" });
    fireEvent.click(within(calendar).getByRole("button", { name: "Сегодня" }));
    await waitFor(() => expect(within(results()).getByRole("button", { name: /^С 21 октября 2026/ })).toBeInTheDocument());
    await waitFor(() => expect(hits()).toHaveLength(2));
    expect(results()).not.toHaveTextContent("Старая книга");
    fireEvent.click(within(results()).getByRole("button", { name: "Сбросить дату «с»" }));
    await waitFor(() => expect(results()).toHaveTextContent("Старая книга"));
  });
});

describe("Поиск: клавиатура и переход к записи", () => {
  it("стрелки выбирают строку (aria-activedescendant), Enter открывает чат на записи с подсветкой и выделенными словами", async () => {
    setup();
    await screen.findByText("Читаю книгу про море");
    type("море");
    await waitFor(() => expect(options()).toHaveLength(1));
    expect(field()).toHaveAttribute("aria-activedescendant", options()[0]?.id);
    expect(options()[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(field(), { key: "Enter" });
    await waitFor(() => expect(screen.queryByRole("region", { name: "Результаты поиска" })).toBeNull());
    const plate = (await screen.findByText("море")).closest("article") as HTMLElement;
    expect(plate).toHaveClass("message--highlighted");
    expect(plate.querySelector("mark")?.textContent).toBe("море");
    expect(field().value).toBe("");
    // Щелчок в ленте снимает подсветку.
    fireEvent.pointerDown(screen.getByRole("region", { name: "Записи" }));
    expect(plate).not.toHaveClass("message--highlighted");
    expect(plate.querySelector("mark")).toBeNull();
  });

  it("стрелка вниз и вверх двигают выбор по кругу границ; щелчок по строке чата открывает чат", async () => {
    setup();
    await screen.findByText("Читаю книгу про море");
    type("книг");
    await waitFor(() => expect(options()).toHaveLength(4));
    fireEvent.keyDown(field(), { key: "ArrowDown" });
    expect(options()[1]).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(field(), { key: "ArrowUp" });
    fireEvent.keyDown(field(), { key: "ArrowUp" });
    expect(options()[0]).toHaveAttribute("aria-selected", "true");
    fireEvent.click(options()[0] as HTMLElement);
    await waitFor(() => expect(screen.getByRole("button", { name: /^Книги/ })).toHaveAttribute("aria-current", "true"));
    expect(screen.queryByRole("region", { name: "Результаты поиска" })).toBeNull();
  });

  it("Esc в поле очищает его и возвращает ленту туда, где был", async () => {
    setup();
    fireEvent.click(await screen.findByRole("button", { name: /^Книги/ }));
    await screen.findByText("Книга о Раст");
    type("молоко");
    await waitFor(() => expect(options()).toHaveLength(1));
    fireEvent.keyDown(field(), { key: "Escape" });
    expect(field().value).toBe("");
    expect(screen.queryByRole("region", { name: "Результаты поиска" })).toBeNull();
    expect(screen.getByRole("button", { name: /^Книги/ })).toHaveAttribute("aria-current", "true");
    expect(screen.getByText("Книга о Раст")).toBeInTheDocument();
  });

  it("⌘F ставит фокус в поле; в свёрнутом сайдбаре значок поиска разворачивает его и ставит фокус", async () => {
    // В jsdom платформа не мак: ⌘F — для мака, на остальных системах Ctrl+F.
    vi.spyOn(navigator, "platform", "get").mockReturnValue("MacIntel");
    const api = setup();
    await screen.findByText("Читаю книгу про море");
    fireEvent.keyDown(window, { key: "f", metaKey: true });
    await waitFor(() => expect(field()).toHaveFocus());
    fireEvent.click(screen.getByRole("button", { name: "Свернуть боковую панель" }));
    await waitFor(() => expect(screen.queryByRole("combobox", { name: "Поиск" })).toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "Поиск" }));
    await waitFor(() => expect(field()).toHaveFocus());
    expect(api.calls).toContain("setSidebarCollapsed false".trim());
  });

  it("по-английски: подписи и пустой результат", async () => {
    setup({}, "en");
    await screen.findByText("Читаю книгу про море");
    const input = screen.getByRole("combobox", { name: "Search" });
    fireEvent.change(input, { target: { value: "zzz" } });
    expect(await screen.findByText("Nothing found.")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Everywhere" })).toBeInTheDocument();
    expect(INBOX_ID).toBeTruthy();
  });
});
