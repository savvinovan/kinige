import { afterEach, describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, type MockApiOptions, mockMessage } from "../../api/MockApi";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// «В этот день» и ответ на настоящем `MainWindow` и `MockApi` (слайс 5). «Сегодня» — суббота 31 октября 2026, 06:12 UTC.
let nowMs = Date.UTC(2026, 9, 31, 6, 12);
const NOW = () => new Date(nowMs);
const entry = (id: string, year: number, month: number, day: number, body: string, extra: Partial<Parameters<typeof mockMessage>[0]> = {}) =>
  mockMessage({
    id,
    bodyMd: body,
    journalDate: `${year}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`,
    createdAt: Date.UTC(year, month - 1, day, 9, 12),
    ...extra,
  });

const MESSAGES = [
  entry("old2023", 2023, 10, 31, "Так будет лучше"),
  entry("old2025", 2025, 10, 31, "Год назад, проснулся рано"),
  entry("old2019", 2019, 10, 31, "Давно"),
  entry("other", 2024, 10, 31, "Другой день, чужой чат", { chatId: "books" }),
  entry("today", 2026, 10, 31, "Сегодняшняя запись"),
];

afterEach(() => {
  nowMs = Date.UTC(2026, 9, 31, 6, 12);
});

function setup(options: MockApiOptions = {}, language: "ru" | "en" = "ru") {
  const api = new MockApi({ now: NOW, language, chats: [{ id: "books", title: "Книги", icon: "📚" }], messages: MESSAGES, ...options });
  const view = render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language });
  return { api, ...view };
}

const card = () => screen.queryByRole("region", { name: "В этот день" });
const field = () => screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement;
const replyButton = (date: string) => screen.getByRole("button", { name: `Ответить на запись ${date}` });
const chatRow = (name: string) => screen.findByRole("button", { name: new RegExp(`^${name}`) });

describe("Карточка «В этот день» в ленте Inbox", () => {
  it("стоит под меткой «Сегодня» перед сегодняшними записями; годы от новых к старым", async () => {
    setup();
    const region = await screen.findByRole("region", { name: "В этот день" });
    const today = screen.getByRole("heading", { level: 2, name: "Сегодня" });
    const article = document.querySelector('[data-message-id="today"]') as HTMLElement;
    expect(today.compareDocumentPosition(region) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(region.compareDocumentPosition(article) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const years = within(region).getAllByRole("heading", { level: 4 }).map((h) => h.textContent);
    expect(years).toEqual(["1 год назад · 31 октября 2025", "3 года назад · 31 октября 2023", "7 лет назад · 31 октября 2019"]);
    expect(within(region).queryByText("Другой день, чужой чат")).toBeNull();
  });

  it("в первом кадре ленты карточка уже есть: лента не прыгает", async () => {
    const api = new MockApi({ now: NOW, messages: MESSAGES });
    const states: boolean[] = [];
    const observer = new MutationObserver(() => {
      if (document.querySelector("article") || document.querySelector(".feed__empty")) {
        states.push(document.querySelector(".on-this-day") !== null);
      }
    });
    observer.observe(document.body, { childList: true, subtree: true });
    render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
    await screen.findByRole("region", { name: "В этот день" });
    observer.disconnect();
    expect(states.length).toBeGreaterThan(0);
    expect(states.every(Boolean), `кадры с записями: ${states.join(",")}`).toBe(true);
    expect(screen.queryByText(/Пока пусто/)).toBeNull();
  });

  it("сегодня записей нет: карточка внизу ленты под меткой «Сегодня»", async () => {
    setup({ messages: MESSAGES.filter((m) => m.id !== "today") });
    const region = await screen.findByRole("region", { name: "В этот день" });
    expect(screen.getByRole("heading", { level: 2, name: "Сегодня" })).toBeInTheDocument();
    expect(region.closest("li")?.nextElementSibling).toBeNull();
  });

  it("в прошлые годы в этот день записей нет — карточки нет; в других чатах её нет никогда", async () => {
    setup({ messages: [entry("a", 2025, 10, 30, "вчера в прошлом году"), entry("t", 2026, 10, 31, "сегодня")] });
    await screen.findByText("сегодня");
    expect(card()).toBeNull();
  });

  it("чат «Книги» без карточки, возврат в Inbox её возвращает", async () => {
    setup({ messages: [...MESSAGES, entry("b-today", 2026, 10, 31, "про книги", { chatId: "books" })] });
    await screen.findByRole("region", { name: "В этот день" });
    fireEvent.click(await chatRow("Книги"));
    await screen.findByText("про книги");
    expect(card()).toBeNull();
    fireEvent.click(await chatRow("Входящие"));
    expect(await screen.findByRole("region", { name: "В этот день" })).toBeInTheDocument();
  });

  it("крестик скрывает карточку до следующего дня журнала: перезапуск её не возвращает, новый день — возвращает", async () => {
    const { api, unmount } = setup({ messages: [...MESSAGES, entry("old2025b", 2025, 11, 1, "завтра в прошлом году")] });
    await screen.findByRole("region", { name: "В этот день" });
    fireEvent.click(screen.getByRole("button", { name: "Скрыть до завтра" }));
    await waitFor(() => expect(card()).toBeNull());
    expect(api.dismissedOnThisDay).toBe("2026-10-31");
    expect(field()).toHaveFocus();
    // Перезапуск окна на том же хранилище.
    unmount();
    render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
    await screen.findByText("Сегодняшняя запись");
    expect(card()).toBeNull();
    // Следующий день журнала (после 04:00): окно получает фокус и перечитывает карточку.
    nowMs = Date.UTC(2026, 10, 1, 9, 0);
    // The window "focus" listener is attached by a passive effect after the first commit; an event dispatched before that is lost
    // (the flake of KINIG-48). An empty async `act` flushes pending effects first.
    await act(async () => {});
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });
    const next = await screen.findByRole("region", { name: "В этот день" });
    expect(within(next).getAllByRole("heading", { level: 4 }).map((h) => h.textContent)).toEqual(["1 год назад · 1 ноября 2025"]);
  });

  it("щелчок по записи карточки открывает ленту на ней: запись подсвечена заливкой", async () => {
    setup();
    const region = await screen.findByRole("region", { name: "В этот день" });
    fireEvent.click(within(region).getByRole("button", { name: /^Открыть запись 31 октября 2023/ }));
    await waitFor(() => expect(document.querySelector('[data-message-id="old2023"] .message--highlighted')).not.toBeNull());
  });

  it("ключи en и ru: английская карточка", async () => {
    setup({}, "en");
    const region = await screen.findByRole("region", { name: "On this day" });
    expect(within(region).getAllByRole("heading", { level: 4 })[0]).toHaveTextContent("1 year ago · October 31, 2025");
  });
});

describe("Ответ «N лет спустя»", () => {
  async function startReply() {
    const view = setup();
    await screen.findByRole("region", { name: "В этот день" });
    fireEvent.click(replyButton("31 октября 2023"));
    return view;
  }

  it("«Ответить» ставит строку над полем и курсор в поле", async () => {
    await startReply();
    const strip = await screen.findByRole("group", { name: "Ответ на запись 31 октября 2023" });
    expect(strip).toHaveTextContent("Ответ на запись · 31 октября 2023");
    expect(strip).toHaveTextContent("Так будет лучше");
    await waitFor(() => expect(field()).toHaveFocus());
  });

  it("отправка: в ленте ответ с цитатой «3 года спустя · 31 октября 2023», строка и поле очищены, на Rust ушёл id записи", async () => {
    const { api } = await startReply();
    await screen.findByRole("group", { name: "Ответ на запись 31 октября 2023" });
    fireEvent.change(field(), { target: { value: "До сих пор так думаю" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    const quote = await screen.findByRole("button", { name: "3 года спустя, ответ на запись 31 октября 2023" });
    expect(quote).toHaveTextContent("Так будет лучше");
    expect(quote.closest("article")).toHaveTextContent("До сих пор так думаю");
    expect(screen.queryByRole("group", { name: /^Ответ на запись/ })).toBeNull();
    expect(field().value).toBe("");
    expect(api.calls).not.toContain("sendMessage");
    expect((await api.getDraft(INBOX_ID)).replyTo).toBeNull();
  });

  it("крестик строки отменяет ответ: запись уходит обычной, текст остаётся", async () => {
    await startReply();
    await screen.findByRole("group", { name: "Ответ на запись 31 октября 2023" });
    fireEvent.change(field(), { target: { value: "просто мысль" } });
    fireEvent.click(screen.getByRole("button", { name: "Отменить ответ" }));
    expect(screen.queryByRole("group", { name: /^Ответ на запись/ })).toBeNull();
    expect(field().value).toBe("просто мысль");
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByText("просто мысль");
    expect(screen.queryByRole("button", { name: /спустя/ })).toBeNull();
  });

  it("запись, на которую отвечают, ушла в корзину: ошибка над полем, текст остаётся, строка говорит «Запись в корзине»", async () => {
    const { api } = await startReply();
    await screen.findByRole("group", { name: "Ответ на запись 31 октября 2023" });
    fireEvent.change(field(), { target: { value: "не уйдёт" } });
    await api.deleteMessage("old2023");
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent("больше нет на месте");
    expect(field().value).toBe("не уйдёт");
    await waitFor(() => expect(screen.getByText("Запись в корзине")).toBeInTheDocument());
    expect(screen.queryByText("не уйдёт", { selector: "article *" })).toBeNull();
  });

  it("строка ответа и текст переживают перезапуск окна", async () => {
    const { api, unmount } = await startReply();
    await screen.findByRole("group", { name: "Ответ на запись 31 октября 2023" });
    fireEvent.change(field(), { target: { value: "недописанное" } });
    await waitFor(async () => expect((await api.getDraft(INBOX_ID)).body).toBe("недописанное"));
    unmount();
    render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
    expect(await screen.findByRole("group", { name: "Ответ на запись 31 октября 2023" })).toBeInTheDocument();
    await waitFor(() => expect(field().value).toBe("недописанное"));
  });

  it("щелчок по цитате открывает старую запись в контексте", async () => {
    const { api } = setup();
    await screen.findByRole("region", { name: "В этот день" });
    await api.sendMessage(INBOX_ID, "Ответ", "old2023");
    fireEvent.click(await chatRow("Книги"));
    fireEvent.click(await chatRow("Входящие"));
    const quote = await screen.findByRole("button", { name: "3 года спустя, ответ на запись 31 октября 2023" });
    fireEvent.click(quote);
    await waitFor(() => expect(document.querySelector('[data-message-id="old2023"] .message--highlighted')).not.toBeNull());
  });

  it("старая запись в корзине — «Запись в корзине»; удалена навсегда — «Исходная запись удалена»; сам ответ остаётся", async () => {
    const { api } = setup();
    await screen.findByRole("region", { name: "В этот день" });
    await api.sendMessage(INBOX_ID, "Ответ остаётся", "old2023");
    await api.deleteMessage("old2023");
    fireEvent.click(await chatRow("Книги"));
    fireEvent.click(await chatRow("Входящие"));
    expect(await screen.findByText("Запись в корзине", { selector: ".reply-quote__label" })).toBeInTheDocument();
    await api.purgeMessage("old2023");
    fireEvent.click(await chatRow("Книги"));
    fireEvent.click(await chatRow("Входящие"));
    expect(await screen.findByText("Исходная запись удалена")).toBeInTheDocument();
    expect(screen.getByText("Ответ остаётся")).toBeInTheDocument();
  });
});

describe("Ответы, импортированные из Telegram", () => {
  const TARGET = entry("tg-1", 2023, 3, 14, "**Анна К.**\nЕдем в субботу?", { chatId: "tg" });
  const REPLY = entry("tg-2", 2023, 3, 14, "Едем", {
    chatId: "tg",
    replyTo: { id: "tg-1", chatId: "tg", journalDate: "2023-03-14", excerpt: "Анна К. Едем в субботу?", state: "live", yearsLater: 0 },
  });
  const NO_TARGET = entry("tg-3", 2023, 3, 15, "Ответ на то, чего не передали", { chatId: "tg" });

  it("цитата без подписи «N лет спустя» с именем автора; ответ на непереданную реплику цитаты не имеет; щелчок открывает реплику", async () => {
    setup({ chats: [{ id: "tg", title: "Анна", icon: "💬" }], messages: [TARGET, REPLY, NO_TARGET] });
    fireEvent.click(await chatRow("Анна"));
    const quote = await screen.findByRole("button", { name: "Ответ на запись 14 марта 2023" });
    expect(quote).toHaveTextContent("Ответ на запись · 14 марта 2023");
    expect(quote).toHaveTextContent("Анна К. Едем в субботу?");
    expect(quote.textContent).not.toMatch(/спустя/);
    expect(screen.getAllByRole("button", { name: /^Ответ на запись/ })).toHaveLength(1);
    fireEvent.click(quote);
    await waitFor(() => expect(document.querySelector('[data-message-id="tg-1"] .message--highlighted')).not.toBeNull());
  });
});
