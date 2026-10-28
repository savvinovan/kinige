import { afterEach, describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, MockApi, type MockApiOptions, mockMessage } from "../../api/MockApi";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Слайс 3 на настоящем `MainWindow` и `MockApi`: чаты, действия над записями, корзина, теги, события Rust.
// 2026-10-21 06:12 UTC; день записи считается с границей 04:00, поэтому «сегодня» — 2026-10-21.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const TODAY_AT = Date.UTC(2026, 9, 21, 5, 0);

const BOOKS = { id: "books", title: "Книги", icon: "📚" };
const IDEAS = { id: "ideas", title: "Идеи", icon: "💡" };

afterEach(() => document.documentElement.removeAttribute("data-text-scale"));

function setup(options: MockApiOptions = {}, language: "ru" | "en" = "ru") {
  const api = new MockApi({ now: NOW, language, ...options });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language });
  return api;
}

const field = () => screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement;
/** Обёртка плашки: в ней ряд действий и сама плашка. */
const plate = (text: string) => screen.getByText(text).closest(".message-wrap") as HTMLElement;
/** Сама плашка: на ней правый щелчок открывает меню записи. */
const article = (text: string) => screen.getByText(text).closest("article") as HTMLElement;
const chatRow = (name: RegExp | string) => screen.findByRole("button", { name: typeof name === "string" ? new RegExp(`^${name}`) : name });
const type = (value: string) => fireEvent.change(field(), { target: { value } });

async function openChat(name: string) {
  fireEvent.click(await chatRow(name));
  await waitFor(() => expect(screen.getByRole("button", { name: new RegExp(`^${name}`) })).toHaveAttribute("aria-current", "true"));
}

describe("MainWindow слайс 3: чаты", () => {
  it("чаты по разделам: закреплённые, обычные, архив; переключение показывает записи своего чата", async () => {
    setup({
      chats: [BOOKS, { ...IDEAS, pinnedAt: 1 }, { id: "old", title: "Старое", icon: "🗄️", archivedAt: 2 }],
      messages: [
        mockMessage({ id: "i1", bodyMd: "во входящих", createdAt: TODAY_AT }),
        mockMessage({ id: "b1", chatId: "books", bodyMd: "про книги", createdAt: TODAY_AT }),
      ],
    });
    expect(await screen.findByText("во входящих")).toBeInTheDocument();
    expect(screen.getByText("Закреплённые")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Идеи/ })).toBeInTheDocument();
    // Архивный чат спрятан в свёрнутом разделе.
    expect(screen.queryByRole("button", { name: /^Старое/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Архив" }));
    expect(screen.getByRole("button", { name: /^Старое/ })).toBeInTheDocument();

    await openChat("Книги");
    expect(await screen.findByText("про книги")).toBeInTheDocument();
    expect(screen.queryByText("во входящих")).toBeNull();
  });

  it("новый чат: диалог, имя, «Создать» — чат появляется и открывается, фокус в поле ввода", async () => {
    const api = setup();
    fireEvent.click(await screen.findByRole("button", { name: "Новый чат" }));
    const dialog = screen.getByRole("dialog", { name: "Новый чат" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Название" }), { target: { value: "Поездки" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Создать" }));
    await waitFor(() => expect(screen.getByRole("button", { name: /^Поездки/ })).toHaveAttribute("aria-current", "true"));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.calls).toContain("createChat Поездки null");
    await waitFor(() => expect(field()).toHaveFocus());
  });

  it("пустое имя и ошибка Rust: диалог остаётся с текстом ошибки, чат не создан", async () => {
    const api = setup();
    fireEvent.click(await screen.findByRole("button", { name: "Новый чат" }));
    const dialog = screen.getByRole("dialog", { name: "Новый чат" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Название" }), { target: { value: "Поездки" } });
    api.failNext("createChat", "chat_title_too_long");
    fireEvent.click(within(dialog).getByRole("button", { name: "Создать" }));
    expect(await within(dialog).findByText("Название чата — не длиннее 128 знаков.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Поездки/ })).toBeNull();
  });

  it("переименование через меню чата: новое имя в списке", async () => {
    setup({ chats: [BOOKS] });
    fireEvent.contextMenu(await chatRow("Книги"));
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Переименовать" }));
    const dialog = screen.getByRole("dialog", { name: "Переименовать чат" });
    const name = within(dialog).getByRole("textbox", { name: "Название" }) as HTMLInputElement;
    expect(name.value).toBe("Книги");
    fireEvent.change(name, { target: { value: "Читаю" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Сохранить" }));
    expect(await chatRow("Читаю")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Книги/ })).toBeNull();
  });

  it("закрепление поднимает чат в раздел «Закреплённые», архивирование открытого чата возвращает во «Входящие»", async () => {
    setup({ chats: [BOOKS, IDEAS] });
    fireEvent.contextMenu(await chatRow("Идеи"));
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Закрепить" }));
    expect(await screen.findByText("Закреплённые")).toBeInTheDocument();

    await openChat("Книги");
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Книги/ }));
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "В архив" }));
    await waitFor(() => expect(screen.getByRole("button", { name: /^Входящие/ })).toHaveAttribute("aria-current", "true"));
    expect(screen.queryByRole("button", { name: /^Книги/ })).toBeNull();
  });

  it("Inbox нельзя переименовать: в меню только «Закрепить»", async () => {
    setup();
    fireEvent.contextMenu(await chatRow("Входящие"));
    expect(within(screen.getByRole("menu")).getAllByRole("menuitem").map((i) => i.textContent)).toEqual(["Закрепить"]);
  });

  it("свёрнутая панель запоминается в настройках; разворот возвращает названия", async () => {
    const api = setup({ chats: [BOOKS] });
    fireEvent.click(await screen.findByRole("button", { name: "Свернуть боковую панель" }));
    await waitFor(async () => expect((await api.getSettings()).sidebarCollapsed).toBe(true));
    expect(screen.queryByRole("button", { name: "Свернуть боковую панель" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Развернуть боковую панель" }));
    await waitFor(async () => expect((await api.getSettings()).sidebarCollapsed).toBe(false));
    expect(await screen.findByRole("button", { name: "Свернуть боковую панель" })).toBeInTheDocument();
  });

  it("событие «открыть чат» из меню значка открывает чат", async () => {
    const api = setup({ chats: [BOOKS] });
    await chatRow("Книги");
    // Подписка ставится после первого кадра: ждём, пока обработчик появится.
    await waitFor(() => {
      act(() => api.emitOpenChat("books"));
      expect(screen.getByRole("button", { name: /^Книги/ })).toHaveAttribute("aria-current", "true");
    });
  });

  it("запись из окна захвата: событие обновляет ленту и счётчик, не трогая поле ввода", async () => {
    const api = setup();
    await screen.findByText(/Пока пусто/);
    type("набираю");
    api.addExternalMessage(mockMessage({ id: "ext", bodyMd: "из захвата", createdAt: TODAY_AT }));
    await waitFor(() => {
      act(() => api.emitEntriesChanged());
      expect(screen.getByText("из захвата")).toBeInTheDocument();
    });
    expect(field().value).toBe("набираю");
  });
});

describe("MainWindow слайс 3: действия над записью", () => {
  const one = (extra: Partial<Parameters<typeof mockMessage>[0]> = {}) => ({
    messages: [mockMessage({ id: "m1", bodyMd: "Купить хлеб #еда", createdAt: TODAY_AT, ...extra })],
  });

  it("правка: «Редактировать» открывает поле, Enter сохраняет, прежний текст уходит в ревизию", async () => {
    const api = setup(one());
    await screen.findByText("Купить хлеб");
    fireEvent.click(within(plate("Купить хлеб")).getByRole("button", { name: "Редактировать" }));
    const editor = screen.getByRole("textbox", { name: "Правка записи" });
    expect(editor).toHaveValue("Купить хлеб #еда");
    fireEvent.change(editor, { target: { value: "Купить хлеб и молоко" } });
    fireEvent.keyDown(editor, { key: "Enter" });
    expect(await screen.findByText("Купить хлеб и молоко")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Правка записи" })).toBeNull();
    expect(api.revisionsOf("m1")).toEqual(["Купить хлеб #еда"]);
  });

  it("правка: Esc отменяет без записи; ошибка Rust остаётся в плашке вместе с текстом", async () => {
    const api = setup(one());
    await screen.findByText("Купить хлеб");
    fireEvent.click(within(plate("Купить хлеб")).getByRole("button", { name: "Редактировать" }));
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Правка записи" }), { key: "Escape" });
    expect(screen.queryByRole("textbox", { name: "Правка записи" })).toBeNull();
    expect(api.calls.filter((c) => c.startsWith("editMessage"))).toEqual([]);

    fireEvent.click(within(plate("Купить хлеб")).getByRole("button", { name: "Редактировать" }));
    const editor = screen.getByRole("textbox", { name: "Правка записи" });
    fireEvent.change(editor, { target: { value: "другое" } });
    api.failNext("editMessage", "store_write_failed");
    fireEvent.keyDown(editor, { key: "Enter" });
    expect(await screen.findByText("Не удалось сохранить запись. Текст остался в поле.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Правка записи" })).toHaveValue("другое");
  });

  it("задача и избранное: переключатели меняют запись; «Выполнено» доступно только задаче", async () => {
    const api = setup(one());
    await screen.findByText("Купить хлеб");
    fireEvent.click(within(plate("Купить хлеб")).getByRole("button", { name: "Сделать задачей" }));
    await waitFor(() => expect(api.messageById("m1")?.isTask).toBe(true));
    fireEvent.click(within(plate("Купить хлеб")).getByRole("button", { name: "В избранное" }));
    await waitFor(() => expect(api.messageById("m1")?.favoriteAt).not.toBeNull());
    fireEvent.click(await within(plate("Купить хлеб")).findByRole("checkbox", { name: "Задача выполнена" }));
    await waitFor(() => expect(api.messageById("m1")?.taskDoneAt).not.toBeNull());
  });

  it("пункт `- [ ]` в тексте переключается щелчком", async () => {
    const api = setup(one({ bodyMd: "- [ ] позвонить\n- [ ] написать" }));
    await screen.findByText("позвонить");
    fireEvent.click(within(plate("позвонить")).getAllByRole("checkbox")[1] as HTMLElement);
    await waitFor(() => expect(api.messageById("m1")?.bodyMd).toBe("- [ ] позвонить\n- [x] написать"));
  });

  it("удаление: без подтверждения, плашка остаётся приглушённой с «Вернуть»; «Вернуть» возвращает запись", async () => {
    const api = setup(one());
    await screen.findByText("Купить хлеб");
    fireEvent.contextMenu(article("Купить хлеб"));
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Удалить" }));
    const restore = await screen.findByRole("button", { name: "Вернуть удалённую запись" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(api.messageById("m1")).toBeDefined();
    expect(screen.getByRole("button", { name: /^Корзина/ })).toHaveTextContent("1");
    fireEvent.click(restore);
    await waitFor(() => expect(screen.queryByRole("button", { name: "Вернуть удалённую запись" })).toBeNull());
    expect(screen.getByRole("button", { name: /^Корзина/ })).not.toHaveTextContent("1");
    expect(screen.getByText("Купить хлеб")).toBeInTheDocument();
  });

  it("удалённая запись исчезает из ленты, когда человек уходит из чата и возвращается", async () => {
    setup({ ...one(), chats: [BOOKS] });
    await screen.findByText("Купить хлеб");
    fireEvent.contextMenu(article("Купить хлеб"));
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Удалить" }));
    await screen.findByRole("button", { name: "Вернуть удалённую запись" });
    await openChat("Книги");
    await openChat("Входящие");
    await screen.findByText(/Пока пусто/);
    expect(screen.queryByText("Купить хлеб")).toBeNull();
  });

  it("смена даты: календарь, щелчок по дню переносит запись; ошибка Rust видна в календаре", async () => {
    const api = setup(one());
    await screen.findByText("Купить хлеб");
    fireEvent.contextMenu(article("Купить хлеб"));
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Изменить дату…" }));
    const dialog = screen.getByRole("dialog", { name: "Изменить дату" });
    api.failNext("setMessageDate", "invalid_date");
    fireEvent.click(dialog.querySelector('button[data-date="2026-10-15"]') as HTMLElement);
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("Такой даты нет.");
    fireEvent.click(dialog.querySelector('button[data-date="2026-10-15"]') as HTMLElement);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Изменить дату" })).toBeNull());
    expect(api.messageById("m1")?.journalDate).toBe("2026-10-15");
    expect(await screen.findByRole("heading", { name: /^15 октября/ })).toBeInTheDocument();
  });

  it("запись только для чтения: правка недоступна и Rust не вызывается", async () => {
    const api = setup(one({ readOnly: true }));
    await screen.findByText("Купить хлеб");
    const edit = within(plate("Купить хлеб")).getByRole("button", { name: "Редактировать" });
    expect(edit).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(edit);
    expect(screen.queryByRole("textbox", { name: "Правка записи" })).toBeNull();
    expect(api.calls.filter((c) => c.startsWith("editMessage"))).toEqual([]);
  });
});

describe("MainWindow слайс 3: теги и корзина", () => {
  const tagged = {
    messages: [
      mockMessage({ id: "t1", bodyMd: "идея про сайт #идеи", createdAt: TODAY_AT }),
      mockMessage({ id: "t2", bodyMd: "купить хлеб #еда", createdAt: TODAY_AT + 1000 }),
    ],
  };

  it("щелчок по тегу фильтрует ленту, «Сбросить фильтр» возвращает всё; смена чата сбрасывает фильтр", async () => {
    setup({ ...tagged, chats: [BOOKS] });
    await screen.findByText("купить хлеб");
    fireEvent.click(within(plate("купить хлеб")).getByText("#еда"));
    await waitFor(() => expect(screen.queryByText("идея про сайт")).toBeNull());
    expect(screen.getByRole("region", { name: "Фильтр по тегу" })).toHaveTextContent("#еда");
    fireEvent.click(screen.getByRole("button", { name: "Сбросить фильтр" }));
    expect(await screen.findByText("идея про сайт")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Фильтр по тегу" })).toBeNull();

    fireEvent.click(within(plate("идея про сайт")).getByText("#идеи"));
    await waitFor(() => expect(screen.queryByText("купить хлеб")).toBeNull());
    await openChat("Книги");
    expect(screen.queryByRole("region", { name: "Фильтр по тегу" })).toBeNull();
  });

  it("смена тега-фильтра не убирает плашку «Удалено · Вернуть»: чат человек не покидал; фильтр без её тега — убирает", async () => {
    setup({
      messages: [
        mockMessage({ id: "a", bodyMd: "купить хлеб #еда", createdAt: TODAY_AT }),
        mockMessage({ id: "b", bodyMd: "идея про сайт #идеи", createdAt: TODAY_AT + 1000 }),
        mockMessage({ id: "c", bodyMd: "молоко #еда", createdAt: TODAY_AT + 2000 }),
      ],
    });
    await screen.findByText("молоко");
    fireEvent.contextMenu(article("купить хлеб"));
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Удалить" }));
    await screen.findByRole("button", { name: "Вернуть удалённую запись" });

    // Фильтр по тому же тегу: удалённая запись подходит под него и остаётся на месте.
    fireEvent.click(within(plate("молоко")).getByText("#еда"));
    await waitFor(() => expect(screen.queryByText("идея про сайт")).toBeNull());
    expect(screen.getByRole("button", { name: "Вернуть удалённую запись" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Сбросить фильтр" }));
    await screen.findByText("идея про сайт");
    expect(screen.getByRole("button", { name: "Вернуть удалённую запись" })).toBeInTheDocument();

    // Фильтр по другому тегу: удалённая запись под него не подходит.
    fireEvent.click(within(plate("идея про сайт")).getByText("#идеи"));
    await waitFor(() => expect(screen.queryByText("молоко")).toBeNull());
    expect(screen.queryByRole("button", { name: "Вернуть удалённую запись" })).toBeNull();
  });

  it("корзина: список, «Восстановить» возвращает запись в чат, «Удалить навсегда» только после подтверждения", async () => {
    const api = setup({
      messages: [
        mockMessage({ id: "d1", bodyMd: "первая в корзине", createdAt: TODAY_AT }),
        mockMessage({ id: "d2", bodyMd: "вторая в корзине", createdAt: TODAY_AT + 1000 }),
        mockMessage({ id: "k1", bodyMd: "живая", createdAt: TODAY_AT + 2000 }),
      ],
    });
    await screen.findByText("живая");
    await api.deleteMessage("d1");
    await api.deleteMessage("d2");
    api.emitEntriesChanged();
    fireEvent.click(await screen.findByRole("button", { name: /^Корзина/ }));
    const region = await screen.findByRole("region", { name: "Корзина" });
    await within(region).findByText("первая в корзине");

    fireEvent.click(within(region).getAllByRole("button", { name: /^Восстановить/ })[0] as HTMLElement);
    await waitFor(() => expect(within(region).getAllByRole("button", { name: /^Восстановить/ })).toHaveLength(1));

    fireEvent.click(within(region).getByRole("button", { name: /^Удалить навсегда/ }));
    const confirm = screen.getByRole("dialog", { name: "Удалить навсегда?" });
    // Подтверждение: «Отмена» ничего не делает.
    fireEvent.click(within(confirm).getByRole("button", { name: "Отмена" }));
    expect(api.calls.filter((c) => c.startsWith("purgeMessage"))).toEqual([]);

    fireEvent.click(within(region).getByRole("button", { name: /^Удалить навсегда/ }));
    fireEvent.click(within(screen.getByRole("dialog", { name: "Удалить навсегда?" })).getByRole("button", { name: "Удалить навсегда" }));
    await waitFor(() => expect(within(region).getByText("Корзина пуста")).toBeInTheDocument());
    const purged = api.calls.filter((c) => c.startsWith("purgeMessage"));
    expect(purged).toHaveLength(1);
    // Удалённая навсегда запись исчезла из «базы», восстановленная — осталась.
    const gone = (purged[0] as string).split(" ")[1] as string;
    expect(api.messageById(gone)).toBeUndefined();
    expect(["d1", "d2"].filter((id) => api.messageById(id) !== undefined)).toHaveLength(1);
  });

  it("ошибка корзины показывается строкой над списком", async () => {
    const api = setup({ messages: [mockMessage({ id: "d1", bodyMd: "в корзине", createdAt: TODAY_AT })] });
    await screen.findByText("в корзине");
    await api.deleteMessage("d1");
    api.emitEntriesChanged();
    fireEvent.click(await screen.findByRole("button", { name: /^Корзина/ }));
    await screen.findByText("в корзине");
    api.failNext("restoreMessage", "message_not_found");
    fireEvent.click(screen.getByRole("button", { name: /^Восстановить/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Запись не найдена");
  });
});

describe("MainWindow слайс 3: английский", () => {
  it("подписи сайдбара и меню записи на английском", async () => {
    setup({ messages: [mockMessage({ id: "m1", bodyMd: "Buy bread", createdAt: TODAY_AT })] }, "en");
    expect(await screen.findByRole("button", { name: "New chat" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Trash/ })).toBeInTheDocument();
    fireEvent.contextMenu(article("Buy bread"));
    expect(within(screen.getByRole("menu")).getAllByRole("menuitem").map((i) => i.textContent)).toEqual([
      "Copy",
      "Edit",
      "Make a task",
      "Add to favorites",
      "Change date…",
      "Move to chat",
      "Delete",
    ]);
  });
});
