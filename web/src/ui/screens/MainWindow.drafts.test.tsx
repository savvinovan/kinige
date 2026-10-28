import { describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, type MockApiOptions, mockMessage } from "../../api/MockApi";
import type { ImportPreview } from "../../api/generated/ImportPreview";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Черновик у каждого чата на настоящем `MainWindow` и `MockApi` (§5.1): переключение чатов и гонки между набором, сохранением
// и ответом на запрос черновика.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

const BOOKS = { id: "books", title: "Книги", icon: "📚" };
const PREVIEW: ImportPreview = {
  chats: [{ id: "1000001", name: null, kind: "saved", total: 3, existing: 0 }],
  skippedChats: 0,
};

describe("MainWindow: черновик у каждого чата", () => {
  it("переключение сохраняет набранное и подставляет его обратно", async () => {
    const api = setup();
    await screen.findByText(/Пока пусто/);
    type("недописанное во входящих");
    await openChat("Книги");
    expect(field().value).toBe("");
    expect(await api.getDraft(INBOX_ID)).toBe("недописанное во входящих");
    type("про книги");
    await openChat("Входящие");
    await waitFor(() => expect(field().value).toBe("недописанное во входящих"));
    expect(await api.getDraft("books")).toBe("про книги");
  });

  it("набор в ту же долю секунды, когда чат открылся, остаётся черновиком этого чата, а не прежнего", async () => {
    const api = setup();
    await screen.findByText(/Пока пусто/);
    type("недописанное во входящих");
    const typed = whenOpened("Книги", () => type("про книги"));
    fireEvent.click(await chatRow("Книги"));
    await typed;
    await openChat("Входящие");
    await waitFor(() => expect(field().value).toBe("недописанное во входящих"));
    expect(await api.getDraft("books")).toBe("про книги");
    expect(await api.getDraft(INBOX_ID)).toBe("недописанное во входящих");
  });

  it("запоздавший ответ с черновиком прежнего чата не попадает в поле нового", async () => {
    const api = new MockApi({ now: NOW, language: "ru", chats: [BOOKS] });
    await api.saveDraft(INBOX_ID, "старый черновик");
    // Ответ на первый запрос черновика Inbox задерживается, пока человек не переключится на другой чат.
    const answer = api.getDraft.bind(api);
    let release: () => void = () => undefined;
    const held = new Promise<void>((resolve) => (release = resolve));
    let reply: Promise<string> = held.then(() => "");
    api.getDraft = (chatId) => {
      if (chatId !== INBOX_ID) return answer(chatId);
      reply = held.then(() => answer(chatId));
      return reply;
    };
    render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "ru" });
    await screen.findByText(/Пока пусто/);

    // Ответ приходит сразу после отрисовки Книг, пока React ещё не сбросил эффект Inbox; поле Книг должно остаться пустым.
    const answered = whenOpened("Книги", () => {
      release();
      return reply;
    });
    fireEvent.click(await chatRow("Книги"));
    await answered;
    await waitFor(() => expect(screen.getByRole("button", { name: /^Книги/ })).toHaveAttribute("aria-current", "true"));
    expect(field().value).toBe("");
  });
});

describe("MainWindow: повторный выбор уже открытого чата не стирает набранное", () => {
  it("щелчок по открытому чату в сайдбаре: текст в поле остаётся, дальнейший набор дописывает черновик", async () => {
    const api = setup();
    await screen.findByText(/Пока пусто/);
    type("важный текст");
    fireEvent.click(await chatRow("Входящие"));
    await savedDraft(api, INBOX_ID, "важный текст");
    expect(field().value).toBe("важный текст");
    type("важный текст, дальше");
    await savedDraft(api, INBOX_ID, "важный текст, дальше");
  });

  it("меню значка «открыть чат» для открытого чата", async () => {
    const api = setup();
    await openChat("Книги");
    type("про книги");
    // Подписка на событие ставится после первого кадра: повторяем, пока событие не сработает на другом чате.
    await waitFor(() => {
      act(() => api.emitOpenChat(INBOX_ID));
      expect(screen.getByRole("button", { name: /^Входящие/ })).toHaveAttribute("aria-current", "true");
    });
    await openChat("Книги");
    await waitFor(() => expect(field().value).toBe("про книги"));
    act(() => api.emitOpenChat("books"));
    await savedDraft(api, "books", "про книги");
    expect(field().value).toBe("про книги");
  });

  it("«Готово» импорта при открытом Inbox", async () => {
    const api = setup();
    api.importFile = PREVIEW;
    await screen.findByText(/Пока пусто/);
    type("недописанное");
    fireEvent.click(await screen.findByRole("button", { name: /^Профиль/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Импорт из Telegram…" }));
    const dialog = await screen.findByRole("dialog", { name: "Импорт из Telegram" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Выбрать result.json…" }));
    await within(dialog).findByRole("list", { name: "Чаты для импорта" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Импортировать" }));
    await within(screen.getByRole("dialog")).findByRole("heading", { name: "Импорт завершён" });
    fireEvent.click(screen.getByRole("button", { name: "Готово" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await savedDraft(api, INBOX_ID, "недописанное");
    expect(field().value).toBe("недописанное");
  });

  it("переход из поиска к записи в открытом чате", async () => {
    const api = setup({ messages: [mockMessage({ id: "m1", bodyMd: "заметка про яблоки", createdAt: Date.UTC(2026, 9, 21, 5, 0) })] });
    await screen.findByText("заметка про яблоки");
    type("недописанное");
    fireEvent.change(screen.getByRole("combobox", { name: "Поиск" }), { target: { value: "яблоки" } });
    fireEvent.click(await screen.findByRole("option", { name: /яблоки/ }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Результаты поиска" })).toBeNull());
    await savedDraft(api, INBOX_ID, "недописанное");
    expect(field().value).toBe("недописанное");
  });

  it("A→B→A двумя щелчками подряд: поле A возвращает свой текст", async () => {
    const api = setup();
    await screen.findByText(/Пока пусто/);
    await chatRow("Книги");
    type("текст А");
    // Оба щелчка до первой отрисовки: React объединяет обновления, `selected` остаётся прежним.
    fireEvent.click(screen.getByRole("button", { name: /^Книги/ }));
    fireEvent.click(screen.getByRole("button", { name: /^Входящие/ }));
    await waitFor(() => expect(screen.getByRole("button", { name: /^Входящие/ })).toHaveAttribute("aria-current", "true"));
    await waitFor(() => expect(field().value).toBe("текст А"));
    await savedDraft(api, INBOX_ID, "текст А");
  });
});

function setup(options: MockApiOptions = {}) {
  const api = new MockApi({ now: NOW, language: "ru", chats: [BOOKS], ...options });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "ru" });
  return api;
}

/** Черновик дошёл до Rust (сохранение идёт с паузой после набора). */
const savedDraft = (api: MockApi, chatId: string, text: string) =>
  waitFor(async () => expect(await api.getDraft(chatId)).toBe(text));

const field = () => screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement;
const chatRow = (name: string) => screen.findByRole("button", { name: new RegExp(`^${name}`) });
const type = (value: string) => fireEvent.change(field(), { target: { value } });

async function openChat(name: string) {
  fireEvent.click(await chatRow(name));
  await waitFor(() => expect(screen.getByRole("button", { name: new RegExp(`^${name}`) })).toHaveAttribute("aria-current", "true"));
}

/**
 * Выполняет `action` в тот миг, когда чат `name` стал открытым в DOM. Колбэк MutationObserver идёт сразу после отрисовки,
 * раньше, чем React сбросит эффекты предыдущего чата; промис результата завершается вместе с `action`. Не дождавшись
 * чата за `timeoutMs`, наблюдатель снимается, а промис отклоняется понятной ошибкой.
 */
function whenOpened(name: string, action: () => unknown, timeoutMs = 4000) {
  return new Promise<void>((resolve, reject) => {
    const observer = new MutationObserver(() => {
      const row = screen.queryByRole("button", { name: new RegExp(`^${name}`) });
      if (row?.getAttribute("aria-current") !== "true") return;
      observer.disconnect();
      clearTimeout(timer);
      try {
        Promise.resolve(action()).then(() => resolve(), reject);
      } catch (error) {
        reject(error);
      }
    });
    const timer = setTimeout(() => {
      observer.disconnect();
      reject(new Error(`Чат «${name}» не стал открытым за ${timeoutMs} мс`));
    }, timeoutMs);
    observer.observe(document.body, { subtree: true, attributes: true, childList: true });
  });
}
