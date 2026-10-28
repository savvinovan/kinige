import { describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import { fireEvent, render, screen, waitFor } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Черновик у каждого чата на настоящем `MainWindow` и `MockApi` (§5.1): переключение чатов и гонки между набором, сохранением
// и ответом на запрос черновика.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

const BOOKS = { id: "books", title: "Книги", icon: "📚" };

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

function setup() {
  const api = new MockApi({ now: NOW, language: "ru", chats: [BOOKS] });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "ru" });
  return api;
}

const field = () => screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement;
const chatRow = (name: string) => screen.findByRole("button", { name: new RegExp(`^${name}`) });
const type = (value: string) => fireEvent.change(field(), { target: { value } });

async function openChat(name: string) {
  fireEvent.click(await chatRow(name));
  await waitFor(() => expect(screen.getByRole("button", { name: new RegExp(`^${name}`) })).toHaveAttribute("aria-current", "true"));
}

/**
 * Выполняет `action` в тот миг, когда чат `name` стал открытым в DOM. Колбэк MutationObserver идёт сразу после отрисовки,
 * раньше, чем React сбросит эффекты предыдущего чата; промис результата завершается вместе с `action`.
 */
function whenOpened(name: string, action: () => unknown) {
  return new Promise<void>((resolve, reject) => {
    const check = () => {
      const row = screen.queryByRole("button", { name: new RegExp(`^${name}`) });
      if (row?.getAttribute("aria-current") !== "true") return;
      observer.disconnect();
      Promise.resolve(action()).then(() => resolve(), reject);
    };
    const observer = new MutationObserver(check);
    observer.observe(document.body, { subtree: true, attributes: true, childList: true });
  });
}
