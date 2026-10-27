import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, MockApi } from "../api/MockApi";
import { installEmojiLayoutStub, StubEmojiData } from "../test/emoji";
import { act, fireEvent, render, screen, waitFor, within } from "../test/render";
import { CaptureWindow, HIDE_REQUEST_EVENT, SHOWN_EVENT } from "./CaptureWindow";

// Эмодзи в окне быстрого захвата (решение приёмки слайса 3): кнопка в поле, пикер в потоке над полем, окно растёт вверх.
let restoreLayout: () => void;
beforeAll(() => {
  restoreLayout = installEmojiLayoutStub();
});
afterAll(() => restoreLayout());

const NOW = new Date(Date.UTC(2026, 9, 21, 6, 12));
const BOOK = "\u{1F4DA}";
const BULB = "\u{1F4A1}";

/** Высота содержимого в jsdom: окно с пикером выше (высоты задают подставленные размеры, не раскладка). */
const CLOSED_HEIGHT = 150;
const OPEN_HEIGHT = 520;

const observers = new Set<() => void>();
let rect: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  observers.clear();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: () => void) {
        observers.add(callback);
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  const original = HTMLElement.prototype.getBoundingClientRect;
  rect = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    if (!this.classList.contains("capture-shell")) return original.call(this);
    return { height: this.querySelector(".emoji-inline") ? OPEN_HEIGHT : CLOSED_HEIGHT } as DOMRect;
  });
});
afterEach(() => {
  rect.mockRestore();
  vi.unstubAllGlobals();
});

async function setup(recent: string[] = [BOOK, BULB]) {
  const api = new MockApi({ now: () => NOW, language: "ru", settings: { ...DEFAULT_SETTINGS, recentEmoji: recent } });
  const initial = await api.captureState();
  render(
    <StubEmojiData>
      <CaptureWindow api={api} initial={initial} />
    </StubEmojiData>,
    { language: "ru" },
  );
  return api;
}

const field = () => screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement;
const button = () => screen.getByRole("button", { name: "Эмодзи" });
const dialog = () => screen.getByRole("dialog", { name: "Выбор эмодзи" });
const open = () => fireEvent.click(button());
/** Страница сообщает высоту после того, как ResizeObserver сработал. */
const flushResize = () =>
  act(() => {
    for (const callback of observers) (callback as (entries: unknown[]) => void)([]);
  });

describe("окно захвата: эмодзи", () => {
  it("в поле есть кнопка «Эмодзи»; пикер встаёт в панель над полем, а не поверх окна", async () => {
    await setup();
    expect(button()).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("dialog")).toBeNull();
    open();
    expect(button()).toHaveAttribute("aria-expanded", "true");
    const panel = dialog();
    expect(panel.closest(".capture-panel")).not.toBeNull();
    expect(panel.parentElement).toHaveClass("composer__emoji-host");
    // Пикер стоит выше строки поля, кнопка и поле остаются на месте.
    const bar = field().closest(".composer__bar") as HTMLElement;
    expect(panel.compareDocumentPosition(bar) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(panel).getByRole("searchbox", { name: "Найти эмодзи" })).toHaveFocus();
  });

  it("пока пикер открыт, окно растёт вверх через capture_resize, при закрытии возвращается", async () => {
    const api = await setup();
    flushResize();
    expect(api.resizeLog).toEqual([CLOSED_HEIGHT]);
    open();
    flushResize();
    expect(api.resizeLog).toEqual([CLOSED_HEIGHT, OPEN_HEIGHT]);
    open();
    flushResize();
    expect(api.resizeLog).toEqual([CLOSED_HEIGHT, OPEN_HEIGHT, CLOSED_HEIGHT]);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("выбор эмодзи вставляет его в позицию курсора, закрывает пикер и возвращает фокус в поле; тишина: ни тоста, ни hide", async () => {
    const api = await setup();
    fireEvent.change(field(), { target: { value: "купить хлеб" } });
    field().setSelectionRange(6, 6);
    open();
    fireEvent.click(within(dialog()).getByRole("button", { name: BOOK }));
    expect(field().value).toBe(`купить${BOOK} хлеб`);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(field()).toHaveFocus();
    expect(field().selectionStart).toBe(6 + BOOK.length);
    expect(api.hideCount).toBe(0);
    expect(document.querySelector(".toast, [role=status]")).toBeNull();
  });

  it("выбранный эмодзи уходит в общий список «недавних»: главное окно видит его у себя", async () => {
    const api = await setup([BOOK, BULB]);
    open();
    fireEvent.click(within(dialog()).getByRole("button", { name: BULB }));
    await waitFor(async () => expect((await api.captureState()).recentEmoji).toEqual([BULB, BOOK]));
    expect((await api.getSettings()).recentEmoji).toEqual([BULB, BOOK]);
    // Следующее открытие показывает его первым в «Недавних».
    open();
    const recent = within(dialog()).getByRole("region", { name: "Недавние" });
    expect(within(recent).getAllByRole("button").map((b) => b.textContent)).toEqual([BULB, BOOK]);
  });

  it("«Недавние» берутся из общего списка при показе окна", async () => {
    const api = await setup([BOOK]);
    await api.addRecentEmoji(BULB);
    act(() => void window.dispatchEvent(new Event(SHOWN_EVENT)));
    await waitFor(() => expect(api.captureState()).resolves.toMatchObject({ recentEmoji: [BULB, BOOK] }));
    open();
    const recent = await within(dialog()).findByRole("region", { name: "Недавние" });
    expect(within(recent).getAllByRole("button").map((b) => b.textContent)).toEqual([BULB, BOOK]);
  });

  it("Esc в пикере закрывает пикер, окно остаётся; второй Esc прячет окно", async () => {
    const api = await setup();
    open();
    fireEvent.keyDown(within(dialog()).getByRole("searchbox"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.hideCount).toBe(0);
    expect(field()).toHaveFocus();
    fireEvent.keyDown(field(), { key: "Escape" });
    await waitFor(() => expect(api.hideCount).toBe(1));
  });

  it("Esc с фокусом в поле при открытом пикере тоже закрывает сначала пикер", async () => {
    const api = await setup();
    open();
    field().focus();
    fireEvent.keyDown(field(), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.hideCount).toBe(0);
    expect(field()).toHaveFocus();
    fireEvent.keyDown(field(), { key: "Escape" });
    await waitFor(() => expect(api.hideCount).toBe(1));
  });

  it("запрос спрятать окно и новый показ закрывают пикер: окно открывается всегда без него", async () => {
    const api = await setup();
    open();
    act(() => void window.dispatchEvent(new Event(HIDE_REQUEST_EVENT)));
    expect(screen.queryByRole("dialog")).toBeNull();
    await waitFor(() => expect(api.hideCount).toBe(1));
    open();
    expect(dialog()).toBeInTheDocument();
    act(() => void window.dispatchEvent(new Event(SHOWN_EVENT)));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("Enter в поле при открытом пикере отправляет запись и закрывает пикер", async () => {
    const api = await setup();
    fireEvent.change(field(), { target: { value: "заметка" } });
    open();
    fireEvent.keyDown(field(), { key: "Enter" });
    await waitFor(() => expect(api.hideCount).toBe(1));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect((await api.captureState()).context.map((m) => m.bodyMd)).toEqual(["заметка"]);
  });

  it("сбой записи в «недавние» не мешает писать: эмодзи уже в поле", async () => {
    const api = await setup();
    api.captureAddRecentEmoji = () => Promise.reject(new Error("нет связи"));
    open();
    fireEvent.click(within(dialog()).getByRole("button", { name: BULB }));
    expect(field().value).toBe(BULB);
    await act(async () => undefined);
    expect(screen.queryByRole("alert", { name: /./ })).toBeNull();
  });

  it("английский: кнопка и панель подписаны по-английски", async () => {
    const api = new MockApi({ now: () => NOW, language: "en" });
    const initial = await api.captureState();
    render(
      <StubEmojiData>
        <CaptureWindow api={api} initial={initial} />
      </StubEmojiData>,
      { language: "en" },
    );
    fireEvent.click(screen.getByRole("button", { name: "Emoji" }));
    expect(screen.getByRole("dialog", { name: "Emoji picker" })).toBeInTheDocument();
  });
});
