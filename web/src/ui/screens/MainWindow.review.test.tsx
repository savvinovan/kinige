import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, type MockApiOptions, mockMessage } from "../../api/MockApi";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Правки по ревью слайса 5: прокрутка к записи один раз, адресат ответа в другом чате, смена дня без карточки.
let nowMs = Date.UTC(2026, 9, 31, 6, 12);
const NOW = () => new Date(nowMs);
const entry = (id: string, year: number, body: string, chatId = INBOX_ID) =>
  mockMessage({ id, bodyMd: body, chatId, journalDate: `${year}-10-31`, createdAt: Date.UTC(year, 9, 31, 9, 12) });

const MESSAGES = [
  entry("old2023", 2023, "Так будет лучше"),
  entry("old2025", 2025, "Год назад, проснулся рано"),
  entry("today", 2026, "Сегодняшняя запись"),
];

const scrollSpy = vi.fn();
beforeEach(() => {
  scrollSpy.mockReset();
  HTMLElement.prototype.scrollIntoView = scrollSpy;
});
afterEach(() => {
  nowMs = Date.UTC(2026, 9, 31, 6, 12);
  Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
});

function setup(options: MockApiOptions = {}) {
  const api = new MockApi({ now: NOW, chats: [{ id: "books", title: "Книги", icon: "📚" }], messages: MESSAGES, ...options });
  const view = render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
  return { api, ...view };
}
const field = () => screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement;

describe("Прокрутка к записи", () => {
  it("лента едет к записи один раз: отправка ответа, появление и скрытие карточки её не возвращают", async () => {
    setup();
    const region = await screen.findByRole("region", { name: "В этот день" });
    fireEvent.click(within(region).getByRole("button", { name: /^Открыть запись 31 октября 2023/ }));
    await waitFor(() => expect(document.querySelector('[data-message-id="old2023"] .message--highlighted')).not.toBeNull());
    expect(scrollSpy).toHaveBeenCalledTimes(1);
    expect(scrollSpy).toHaveBeenCalledWith({ block: "center" });

    fireEvent.click(within(screen.getByRole("region", { name: "В этот день" })).getByRole("button", { name: "Ответить на запись 31 октября 2023" }));
    await screen.findByRole("group", { name: "Ответ на запись 31 октября 2023" });
    fireEvent.change(field(), { target: { value: "До сих пор так думаю" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByRole("button", { name: "3 года спустя, ответ на запись 31 октября 2023" });
    expect(scrollSpy, "после отправки ответа лента не уезжает к старой записи").toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Скрыть до завтра" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "В этот день" })).toBeNull());
    expect(scrollSpy, "после скрытия карточки лента остаётся внизу").toHaveBeenCalledTimes(1);
  });

  it("запрос прокрутки не переживает уход из ленты: после корзины и возврата лента открывается внизу", async () => {
    setup();
    const region = await screen.findByRole("region", { name: "В этот день" });
    fireEvent.click(within(region).getByRole("button", { name: /^Открыть запись 31 октября 2023/ }));
    await waitFor(() => expect(scrollSpy).toHaveBeenCalledTimes(1));
    // Корзина заменяет ленту целиком (лента размонтируется): запрос не должен ждать в состоянии и сработать на новой ленте.
    fireEvent.click(await screen.findByRole("button", { name: /^Корзина/ }));
    await waitFor(() => expect(document.querySelector(".feed-scroll")).toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: /^Входящие/ }));
    await screen.findByText("Сегодняшняя запись");
    expect(scrollSpy).toHaveBeenCalledTimes(1);
  });

  it("запись, которой ещё нет в ленте, ждёт её появления, найдя — прокручивает один раз; смена чата просьбу сбрасывает", async () => {
    const { api } = setup();
    const region = await screen.findByRole("region", { name: "В этот день" });
    fireEvent.click(within(region).getByRole("button", { name: /^Открыть запись 31 октября 2025/ }));
    await waitFor(() => expect(scrollSpy).toHaveBeenCalledTimes(1));
    // Уход в другой чат и возврат: лента открывается внизу, к старой записи не прокручивается.
    fireEvent.click(await screen.findByRole("button", { name: /^Книги/ }));
    await api.sendMessage(INBOX_ID, "новая запись", null);
    fireEvent.click(await screen.findByRole("button", { name: /^Входящие/ }));
    await screen.findByText("новая запись");
    expect(scrollSpy).toHaveBeenCalledTimes(1);
  });
});

describe("Скрытие карточки", () => {
  it("отказ Rust: карточка остаётся, лента никуда не едет", async () => {
    const { api } = setup();
    await screen.findByRole("region", { name: "В этот день" });
    const scroller = document.querySelector(".feed-scroll") as HTMLElement;
    let top = -300;
    Object.defineProperty(scroller, "scrollTop", { configurable: true, get: () => top, set: (value: number) => (top = value) });
    api.failNext("dismissOnThisDay", "store_write_failed");
    fireEvent.click(screen.getByRole("button", { name: "Скрыть до завтра" }));
    await waitFor(() => expect(api.calls).toContain("dismissOnThisDay 2026-10-31"));
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.getByRole("region", { name: "В этот день" })).toBeInTheDocument();
    expect(top).toBe(-300);
    expect(api.dismissedOnThisDay).toBeNull();
  });

  it("после крестика лента возвращается к нижнему краю (область column-reverse: scrollTop = 0), а не остаётся на карточке", async () => {
    setup();
    await screen.findByRole("region", { name: "В этот день" });
    // Читатель прокрутил ленту вверх, к карточке (в column-reverse это отрицательный scrollTop).
    const scroller = document.querySelector(".feed-scroll") as HTMLElement;
    let top = -300;
    Object.defineProperty(scroller, "scrollTop", { configurable: true, get: () => top, set: (value: number) => (top = value) });
    fireEvent.click(screen.getByRole("button", { name: "Скрыть до завтра" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "В этот день" })).toBeNull());
    await waitFor(() => expect(top).toBe(0));
  });
});

describe("Адресат ответа перенесён в другой чат", () => {
  it("отправка снимает ответ: ошибка один раз, строка говорит «перенесена», вторая отправка уходит обычной записью", async () => {
    const { api } = setup();
    const region = await screen.findByRole("region", { name: "В этот день" });
    fireEvent.click(within(region).getByRole("button", { name: "Ответить на запись 31 октября 2023" }));
    await screen.findByRole("group", { name: "Ответ на запись 31 октября 2023" });
    fireEvent.change(field(), { target: { value: "ответ" } });
    await api.moveMessage("old2023", "books");
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent("больше нет на месте");
    expect(await screen.findByText("Запись перенесена в другой чат")).toBeInTheDocument();
    expect(field().value).toBe("ответ");
    // Вторая отправка: без ответа, без ошибки, строка и текст очищены.
    fireEvent.keyDown(field(), { key: "Enter" });
    await waitFor(() => expect(field().value).toBe(""));
    expect(screen.queryByText("Запись перенесена в другой чат")).toBeNull();
    expect(screen.queryByRole("button", { name: /спустя/ })).toBeNull();
    expect((await api.getDraft(INBOX_ID)).replyTo).toBeNull();
  });

  it("запись вернули в тот же чат: при фокусе окна строка снова обычная", async () => {
    const api = new MockApi({ now: NOW, chats: [{ id: "books", title: "Книги", icon: "📚" }], messages: MESSAGES });
    await api.saveDraft(INBOX_ID, "ответ", "old2023");
    await api.moveMessage("old2023", "books");
    render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
    await screen.findByText("Запись перенесена в другой чат");
    await api.moveMessage("old2023", INBOX_ID);
    // The window "focus" listener is attached by a passive effect once the «moved» row is committed. `findByText` can resolve
    // before React has flushed that effect (it runs in a scheduler task, racing with the timers of testing-library), and
    // an event dispatched earlier is lost. An empty async `act` flushes pending effects first.
    await act(async () => {});
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(await screen.findByRole("group", { name: "Ответ на запись 31 октября 2023" })).toBeInTheDocument();
    expect(screen.queryByText("Запись перенесена в другой чат")).toBeNull();
  });

  it("черновик, открытый заново, сразу показывает «перенесена»", async () => {
    const api = new MockApi({ now: NOW, chats: [{ id: "books", title: "Книги", icon: "📚" }], messages: MESSAGES });
    await api.saveDraft(INBOX_ID, "ответ", "old2023");
    await api.moveMessage("old2023", "books");
    render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
    expect(await screen.findByText("Запись перенесена в другой чат")).toBeInTheDocument();
    await waitFor(() => expect(field().value).toBe("ответ"));
  });
});

describe("Новый день журнала при открытом окне", () => {
  it("без карточки: фокус окна перечитывает ленту, «Сегодня» становится «Вчера»", async () => {
    setup({ messages: [entry("t", 2026, "Сегодняшняя запись")] });
    await screen.findByText("Сегодняшняя запись");
    expect(screen.queryByRole("region", { name: "В этот день" })).toBeNull();
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Сегодня");
    nowMs = Date.UTC(2026, 10, 1, 9, 0);
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Вчера"));
  });

  it("карточка скрыта на сегодня: новый день всё равно обновляет метки", async () => {
    const { api } = setup();
    await screen.findByRole("region", { name: "В этот день" });
    await api.dismissOnThisDay("2026-10-31");
    fireEvent.click(await screen.findByRole("button", { name: /^Книги/ }));
    fireEvent.click(await screen.findByRole("button", { name: /^Входящие/ }));
    await screen.findByText("Сегодняшняя запись");
    expect(screen.queryByRole("region", { name: "В этот день" })).toBeNull();
    nowMs = Date.UTC(2026, 10, 1, 9, 0);
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    await waitFor(() => expect(screen.getAllByRole("heading", { level: 2 }).at(-1)).toHaveTextContent("Вчера"));
  });
});
