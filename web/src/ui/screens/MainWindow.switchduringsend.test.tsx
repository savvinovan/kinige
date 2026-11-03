import { describe, expect, it, vi } from "vitest";
import { INBOX_ID, KinigeApiError } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import type { MessageView } from "../../api/generated/MessageView";
import { fireEvent, render, screen, waitFor } from "../../test/render";
import { MainWindow } from "./MainWindow";

// The person switched chats while sending (KINIG-45): the reply to the send belongs to the previous chat and must not touch
// the new chat's composer, draft and error.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const BOOKS = { id: "books", title: "Книги", icon: "📚" };
const BOOKS_DRAFT = "черновик про книги";

const field = () => screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement;

/** Text typed in Inbox, "Send" pressed (the reply is delayed), "Books" opened with a saved draft. */
async function sendThenSwitch() {
  const api = new MockApi({ now: NOW, language: "ru", chats: [BOOKS] });
  await api.saveDraft(BOOKS.id, BOOKS_DRAFT);
  const answer = api.sendMessage.bind(api);
  let finish: (outcome: "ok" | "error") => void = () => undefined;
  const held = new Promise<"ok" | "error">((resolve) => (finish = resolve));
  vi.spyOn(api, "sendMessage").mockImplementation(async (chatId, body, replyToId) => {
    if ((await held) === "error") throw new KinigeApiError("internal");
    return answer(chatId, body, replyToId);
  });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "ru" });
  await screen.findByText(/Пока пусто/);
  fireEvent.change(field(), { target: { value: "мысль для входящих" } });
  fireEvent.click(screen.getByRole("button", { name: "Отправить" }));
  fireEvent.click(await screen.findByRole("button", { name: /^Книги/ }));
  await waitFor(() => expect(field().value).toBe(BOOKS_DRAFT));
  return { api, finish };
}

describe("Переключение чата во время отправки", () => {
  it("успех: запись ушла во «Входящие», поле и черновик «Книг» остались как были", async () => {
    const { api, finish } = await sendThenSwitch();
    finish("ok");
    await waitFor(() => expect(api.messageById("mock-1")).toBeDefined());
    // The reply to the send is handled (chat counters reread); the "Books" composer is intact.
    await waitFor(() => expect(field().disabled).toBe(false));
    expect(field().value).toBe(BOOKS_DRAFT);
    expect((await api.getDraft(BOOKS.id)).body).toBe(BOOKS_DRAFT);
    const sent: MessageView | undefined = api.messageById("mock-1");
    expect(sent?.chatId).toBe(INBOX_ID);
  });

  it("ошибка: набранный текст возвращается в черновик «Входящих», в поле и черновик «Книг» он не попадает", async () => {
    const { api, finish } = await sendThenSwitch();
    finish("error");
    await waitFor(async () => expect((await api.getDraft(INBOX_ID)).body).toBe("мысль для входящих"));
    expect(field().value).toBe(BOOKS_DRAFT);
    expect((await api.getDraft(BOOKS.id)).body).toBe(BOOKS_DRAFT);
    // The send error from Inbox is not shown in the open "Books".
    expect(screen.queryByText("Что-то пошло не так.")).toBeNull();
  });
});
