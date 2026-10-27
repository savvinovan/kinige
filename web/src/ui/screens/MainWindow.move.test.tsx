import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, type MockApiOptions, mockMessage } from "../../api/MockApi";
import { rect } from "../../test/rect";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Перенос записи в другой чат (решение приёмки): подменю «Переместить в чат» и перетаскивание на чат в сайдбаре.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const TODAY_AT = Date.UTC(2026, 9, 21, 5, 0);
const CHATS = [
  { id: "books", title: "Книги", icon: "📚" },
  { id: "ideas", title: "Идеи", icon: "💡", pinnedAt: 1 },
  { id: "old", title: "Очень длинное название старого чата, которое не поместится в меню", icon: "🗄️", archivedAt: 2 },
];

function setup(options: MockApiOptions = {}, collapsed = false) {
  const api = new MockApi({
    now: NOW,
    chats: CHATS,
    messages: [mockMessage({ id: "m1", bodyMd: "Купить хлеб", createdAt: TODAY_AT })],
    ...options,
  });
  render(<MainWindow api={api} initialSettings={{ ...DEFAULT_SETTINGS, sidebarCollapsed: collapsed }} />);
  return api;
}

const article = (text: string) => screen.getByText(text).closest("article") as HTMLElement;
const menu = () => screen.getByRole("menu");
const item = (name: string | RegExp) => within(menu()).getByRole("menuitem", { name });

async function openMoveMenu() {
  await screen.findByText("Купить хлеб");
  fireEvent.contextMenu(article("Купить хлеб"));
  fireEvent.click(item("Переместить в чат"));
}

describe("«Переместить в чат»: меню записи", () => {
  it("подменю: «Назад», закреплённые, обычные и архивные чаты; текущий недоступен, длинное имя обрезается стилем", async () => {
    setup();
    await openMoveMenu();
    expect(screen.getByRole("menu")).toHaveAccessibleName("Переместить в чат: выберите чат");
    const names = within(menu()).getAllByRole("menuitem").map((i) => i.textContent);
    expect(names).toEqual(["Назад", "💡Идеи", "📥Входящие", "📚Книги", "🗄️Очень длинное название старого чата, которое не поместится в меню"]);
    expect(item(/Входящие/)).toHaveAttribute("aria-disabled", "true");
    expect(item(/Идеи/)).not.toHaveAttribute("aria-disabled");
    // Архивный чат отделён чертой от обычных; название лежит в элементе с многоточием (CSS `context-menu__label`).
    expect(item(/Очень длинное/).parentElement).toHaveClass("context-menu__group");
    expect(item(/Очень длинное/).querySelector(".context-menu__label")).toBeInTheDocument();
  });

  it("выбор чата переносит запись: она исчезает из ленты без тоста, счётчики и данные обновляются", async () => {
    const api = setup();
    await openMoveMenu();
    fireEvent.click(item(/Книги/));
    await waitFor(() => expect(screen.queryByText("Купить хлеб")).toBeNull());
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(api.messageById("m1")?.chatId).toBe("books");
    const books = await screen.findByRole("button", { name: /^Книги/ });
    expect(books).toHaveTextContent("1");
    // В «Книгах» запись на месте.
    fireEvent.click(books);
    expect(await screen.findByText("Купить хлеб")).toBeInTheDocument();
  });

  it("в архивный чат переносить можно", async () => {
    const api = setup();
    await openMoveMenu();
    fireEvent.click(item(/Очень длинное/));
    await waitFor(() => expect(api.messageById("m1")?.chatId).toBe("old"));
  });

  it("с клавиатуры: Enter на пункте открывает подменю, фокус на «Назад»; стрелка влево и Esc возвращают, второй Esc закрывает", async () => {
    setup();
    await screen.findByText("Купить хлеб");
    fireEvent.contextMenu(article("Купить хлеб"));
    for (let i = 0; i < 5; i++) fireEvent.keyDown(menu(), { key: "ArrowDown" });
    expect(item("Переместить в чат")).toHaveFocus();
    fireEvent.click(document.activeElement as HTMLElement);
    await waitFor(() => expect(item("Назад")).toHaveFocus());
    // Текущий чат стрелки пропускают: первый после «Назад» — «Идеи».
    fireEvent.keyDown(menu(), { key: "ArrowDown" });
    expect(item(/Идеи/)).toHaveFocus();

    fireEvent.keyDown(menu(), { key: "ArrowLeft" });
    expect(menu()).toHaveAccessibleName("Меню записи");
    expect(item("Переместить в чат")).toBeInTheDocument();
    fireEvent.click(item("Переместить в чат"));
    fireEvent.keyDown(menu(), { key: "Escape" });
    expect(menu()).toHaveAccessibleName("Меню записи");
    fireEvent.keyDown(menu(), { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("ошибка Rust приходит текстом над полем, запись остаётся на месте", async () => {
    const api = setup();
    await openMoveMenu();
    api.failNext("moveMessage", "chat_not_found");
    fireEvent.click(item(/Книги/));
    expect(await screen.findByText("Чат не найден.")).toBeInTheDocument();
    expect(screen.getByText("Купить хлеб")).toBeInTheDocument();
  });

  it("запись, ушедшая в корзину (например, из другого окна), даёт отдельный текст, а не «не найдена»", async () => {
    const api = setup();
    await openMoveMenu();
    api.failNext("moveMessage", "message_in_trash");
    fireEvent.click(item(/Книги/));
    expect(await screen.findByText("Запись в корзине: сначала верните её, потом переносите.")).toBeInTheDocument();
    expect(screen.queryByText(/удалили навсегда/)).toBeNull();
  });

  it("MockApi: перенос записи из корзины — message_in_trash, не message_not_found", async () => {
    const api = setup();
    await screen.findByText("Купить хлеб");
    await api.deleteMessage("m1");
    await expect(api.moveMessage("m1", "books")).rejects.toMatchObject({ code: "message_in_trash" });
    await expect(api.moveMessage("нет-такой", "books")).rejects.toMatchObject({ code: "message_not_found" });
  });

  it("запись только для чтения не переносится: пункт недоступен", async () => {
    setup({ messages: [mockMessage({ id: "m1", bodyMd: "Купить хлеб", createdAt: TODAY_AT, readOnly: true })] });
    await screen.findByText("Купить хлеб");
    fireEvent.contextMenu(article("Купить хлеб"));
    expect(item("Переместить в чат")).toHaveAttribute("aria-disabled", "true");
  });
});

describe("«Переместить в чат»: перетаскивание на чат", () => {
  const original = document.elementFromPoint;
  let under: Element | null = null;
  beforeEach(() => {
    under = null;
    document.elementFromPoint = () => under;
  });
  afterEach(() => {
    document.elementFromPoint = original;
  });

  /** Плашка с настоящими (подставленными) границами: jsdom границ не знает. */
  async function plate() {
    await screen.findByText("Купить хлеб");
    const element = article("Купить хлеб");
    const box = rect(400, 300, 200, 40);
    element.getBoundingClientRect = () => box;
    return element;
  }
  const pointer = (type: "pointerDown" | "pointerMove" | "pointerUp", target: Element | Document, x: number, y: number) =>
    fireEvent[type](target, { button: 0, pointerId: 1, pointerType: "mouse", clientX: x, clientY: y });

  it("бросок плашки на строку чата: чат подсвечивается, запись переезжает, лента не прыгает, тоста нет", async () => {
    const api = setup();
    const element = await plate();
    const row = await screen.findByRole("button", { name: /^Книги/ });
    pointer("pointerDown", element, 450, 320);
    under = row;
    pointer("pointerMove", document, 100, 320);
    expect(row).toHaveClass("chat-row--drop");
    expect(document.querySelector(".record-ghost")).toBeInTheDocument();
    pointer("pointerUp", document, 100, 320);
    await waitFor(() => expect(screen.queryByText("Купить хлеб")).toBeNull());
    expect(api.messageById("m1")?.chatId).toBe("books");
    expect(document.querySelector(".record-ghost")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("в свёрнутом сайдбаре бросок на значок чата", async () => {
    const api = setup({}, true);
    const element = await plate();
    const icon = await screen.findByRole("button", { name: /^Книги/ });
    expect(icon).toHaveClass("rail-button");
    pointer("pointerDown", element, 450, 320);
    under = icon;
    pointer("pointerMove", document, 20, 320);
    expect(icon).toHaveClass("rail-button--drop");
    pointer("pointerUp", document, 20, 320);
    await waitFor(() => expect(api.messageById("m1")?.chatId).toBe("books"));
  });

  it("бросок мимо, на текущий чат и Esc ничего не меняют", async () => {
    const api = setup();
    const element = await plate();
    pointer("pointerDown", element, 450, 320);
    pointer("pointerMove", document, 700, 500);
    pointer("pointerUp", document, 700, 500);

    const inbox = await screen.findByRole("button", { name: /^Входящие/ });
    pointer("pointerDown", element, 450, 320);
    under = inbox;
    pointer("pointerMove", document, 100, 200);
    expect(inbox).not.toHaveClass("chat-row--drop");
    pointer("pointerUp", document, 100, 200);

    const books = await screen.findByRole("button", { name: /^Книги/ });
    pointer("pointerDown", element, 450, 320);
    under = books;
    pointer("pointerMove", document, 100, 320);
    fireEvent.keyDown(document.body, { key: "Escape" });
    pointer("pointerUp", document, 100, 320);

    await act(async () => undefined);
    expect(api.messageById("m1")?.chatId).toBe(INBOX_ID);
    expect(screen.getByText("Купить хлеб")).toBeInTheDocument();
    expect(api.calls.filter((c) => c.startsWith("moveMessage"))).toEqual([]);
  });

  it("в режиме правки и у записи только для чтения плашка не тащится", async () => {
    setup({
      messages: [
        mockMessage({ id: "m1", bodyMd: "Купить хлеб", createdAt: TODAY_AT }),
        mockMessage({ id: "m2", bodyMd: "Чужая запись", createdAt: TODAY_AT + 1000, readOnly: true }),
      ],
    });
    const element = await plate();
    const readOnly = article("Чужая запись");
    readOnly.getBoundingClientRect = element.getBoundingClientRect;
    const row = await screen.findByRole("button", { name: /^Книги/ });
    under = row;
    pointer("pointerDown", readOnly, 450, 320);
    pointer("pointerMove", document, 100, 320);
    expect(document.querySelector(".record-ghost")).toBeNull();
    pointer("pointerUp", document, 100, 320);

    fireEvent.click(within(element.closest(".message-wrap") as HTMLElement).getByRole("button", { name: "Редактировать" }));
    const editing = (await screen.findByRole("textbox", { name: "Правка записи" })).closest("article") as HTMLElement;
    editing.getBoundingClientRect = element.getBoundingClientRect;
    pointer("pointerDown", editing, 450, 320);
    pointer("pointerMove", document, 100, 320);
    expect(document.querySelector(".record-ghost")).toBeNull();
    pointer("pointerUp", document, 100, 320);
  });
});
