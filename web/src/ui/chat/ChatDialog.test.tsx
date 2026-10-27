import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { installEmojiLayoutStub, StubEmojiData } from "../../test/emoji";
import { fireEvent, render, screen, within } from "../../test/render";
import { CHAT_NAME_MAX, ChatDialog, type ChatDialogProps } from "./ChatDialog";

let restoreLayout: () => void;
beforeAll(() => {
  restoreLayout = installEmojiLayoutStub();
});
afterAll(() => restoreLayout());

const BOOK = "\u{1F4DA}";
const BULB = "\u{1F4A1}";

const setup = (props: Partial<ChatDialogProps> = {}, language: "ru" | "en" = "ru") => {
  const onSubmit = vi.fn();
  const onClose = vi.fn();
  const view = render(
    <StubEmojiData>
      <ChatDialog mode="create" recentEmoji={[BOOK, BULB]} language={language} onSubmit={onSubmit} onClose={onClose} {...props} />
    </StubEmojiData>,
    { language },
  );
  return { onSubmit, onClose, ...view };
};

const nameField = () => screen.getByRole("textbox", { name: "Название" }) as HTMLInputElement;
const submitForm = () => fireEvent.submit(nameField().closest("form") as HTMLFormElement);
const type = (value: string) => fireEvent.change(nameField(), { target: { value } });
const submitButton = (label = "Создать") => screen.getByRole("button", { name: label });

describe("ChatDialog: создание", () => {
  it("окно с заголовком, полем «Название» в фокусе, кнопками «Создать» и «Отмена»", () => {
    setup();
    expect(screen.getByRole("dialog", { name: "Новый чат" })).toBeInTheDocument();
    expect(nameField()).toHaveFocus();
    expect(nameField()).toHaveValue("");
    expect(screen.getByRole("button", { name: "Отмена" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Выбрать эмодзи чата" })).toHaveTextContent("\u{1F4AC}");
  });

  it("пустое и пробельное имя: «Создать» недоступна и форма ничего не отправляет", () => {
    const { onSubmit } = setup();
    expect(submitButton()).toBeDisabled();
    type("   ");
    expect(submitButton()).toBeDisabled();
    submitForm();
    expect(onSubmit).not.toHaveBeenCalled();
    type("Книги");
    expect(submitButton()).toBeEnabled();
  });

  it("отправка без эмодзи: имя без пробелов по краям и emoji = null (Rust подставит 💬)", () => {
    const { onSubmit } = setup();
    type("  Книги  ");
    submitForm();
    expect(onSubmit).toHaveBeenCalledExactlyOnceWith("Книги", null);
  });

  it("кнопка «Создать» отправляет форму (она вне <form> и связана атрибутом form)", () => {
    const { onSubmit } = setup();
    type("Книги");
    fireEvent.click(submitButton());
    expect(onSubmit).toHaveBeenCalledExactlyOnceWith("Книги", null);
  });

  it("выбор эмодзи: пикер раскрывается под полем, выбор закрывает его, кнопка показывает эмодзи, фокус в поле", () => {
    const { onSubmit } = setup();
    type("Книги");
    const toggle = screen.getByRole("button", { name: "Выбрать эмодзи чата" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("searchbox", { name: "Найти эмодзи" })).toHaveFocus();
    fireEvent.click(within(screen.getByRole("group", { name: "Эмодзи" })).getByRole("button", { name: BOOK }));
    expect(screen.queryByRole("searchbox")).toBeNull();
    expect(screen.getByRole("button", { name: `Эмодзи чата: ${BOOK}` })).toHaveTextContent(BOOK);
    expect(nameField()).toHaveFocus();
    submitForm();
    expect(onSubmit).toHaveBeenCalledExactlyOnceWith("Книги", BOOK);
  });

  it("Esc в пикере закрывает только пикер и возвращает фокус на кнопку; следующий Esc закрывает окно", () => {
    const { onClose } = setup();
    const toggle = screen.getByRole("button", { name: "Выбрать эмодзи чата" });
    fireEvent.click(toggle);
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Escape" });
    expect(screen.queryByRole("searchbox")).toBeNull();
    expect(toggle).toHaveFocus();
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(toggle, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("«Отмена», крестик и Esc закрывают окно", () => {
    const { onClose } = setup();
    fireEvent.click(screen.getByRole("button", { name: "Отмена" }));
    fireEvent.click(screen.getByRole("button", { name: "Закрыть" }));
    fireEvent.keyDown(nameField(), { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(3);
  });

  it(`имя длиннее ${CHAT_NAME_MAX} знаков обрезается по символам, а не по кодовым единицам`, () => {
    setup();
    type("я".repeat(200));
    expect(Array.from(nameField().value)).toHaveLength(CHAT_NAME_MAX);
    type(BOOK.repeat(150));
    expect(Array.from(nameField().value)).toHaveLength(CHAT_NAME_MAX);
    expect(nameField().value.endsWith(BOOK)).toBe(true);
  });

  it("ошибка Rust: строка role=alert, поле помечено недопустимым, введённое остаётся", () => {
    setup({ error: "Введите название чата." });
    type("x");
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Введите название чата.");
    expect(nameField()).toHaveAttribute("aria-invalid", "true");
    expect(nameField().getAttribute("aria-describedby")).toBe(alert.id);
    expect(nameField()).toHaveValue("x");
  });

  it("запрос идёт: поле, кнопка эмодзи и «Создать» недоступны, отправка не повторяется", () => {
    const { onSubmit } = setup({ busy: true });
    expect(nameField()).toBeDisabled();
    expect(submitButton()).toBeDisabled();
    expect(screen.getByRole("button", { name: "Выбрать эмодзи чата" })).toBeDisabled();
    submitForm();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("длинное имя с кириллицей и латиницей помещается в поле как есть", () => {
    const { onSubmit } = setup();
    const long = "Подготовка к собеседованию по системному дизайну and distributed systems";
    type(long);
    submitForm();
    expect(onSubmit).toHaveBeenCalledWith(long, null);
  });

  it("английский интерфейс", () => {
    setup({}, "en");
    expect(screen.getByRole("dialog", { name: "New chat" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Name" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choose chat emoji" })).toBeInTheDocument();
  });
});

describe("ChatDialog: переименование", () => {
  it("имя подставлено, кнопки эмодзи нет, «Сохранить» отправляет emoji = null", () => {
    const { onSubmit } = setup({ mode: "rename", initialName: "Книги", initialEmoji: BOOK });
    expect(screen.getByRole("dialog", { name: "Переименовать чат" })).toBeInTheDocument();
    expect(nameField()).toHaveValue("Книги");
    expect(nameField()).toHaveFocus();
    expect(screen.queryByRole("button", { name: /эмодзи/i })).toBeNull();
    type("Книги 2026");
    fireEvent.click(submitButton("Сохранить"));
    expect(onSubmit).toHaveBeenCalledExactlyOnceWith("Книги 2026", null);
  });

  it("очистка имени отключает «Сохранить»", () => {
    setup({ mode: "rename", initialName: "Книги" });
    type("");
    expect(submitButton("Сохранить")).toBeDisabled();
  });
});

describe("ChatDialog: смена эмодзи", () => {
  it("только пикер: поля нет, фокус в поиске, выбор сразу отправляет имя и эмодзи", () => {
    const { onSubmit } = setup({ mode: "icon", initialName: "Книги", initialEmoji: BOOK });
    expect(screen.getByRole("dialog", { name: "Эмодзи чата" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Название" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Сохранить" })).toBeNull();
    expect(screen.getByRole("searchbox", { name: "Найти эмодзи" })).toHaveFocus();
    fireEvent.click(within(screen.getByRole("group", { name: "Эмодзи" })).getByRole("button", { name: BULB }));
    expect(onSubmit).toHaveBeenCalledExactlyOnceWith("Книги", BULB);
  });

  it("Esc закрывает окно; ошибка — строка role=alert; во время запроса выбор не отправляется", () => {
    const { onClose, onSubmit, rerender } = setup({ mode: "icon", initialName: "Книги", error: "Этот эмодзи не подошёл. Выберите другой." });
    expect(screen.getByRole("alert")).toHaveTextContent("Этот эмодзи не подошёл");
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
    rerender(
      <StubEmojiData>
        <ChatDialog mode="icon" initialName="Книги" recentEmoji={[BOOK]} language="ru" busy onSubmit={onSubmit} onClose={onClose} />
      </StubEmojiData>,
    );
    fireEvent.click(within(screen.getByRole("group", { name: "Эмодзи" })).getByRole("button", { name: BOOK }));
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("английский заголовок", () => {
    setup({ mode: "icon", initialName: "Books" }, "en");
    expect(screen.getByRole("dialog", { name: "Chat emoji" })).toBeInTheDocument();
  });
});
