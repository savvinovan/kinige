import { useState } from "react";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { installEmojiLayoutStub, StubEmojiData } from "../../test/emoji";
import { fireEvent, render, screen, within } from "../../test/render";
import { Composer, type ComposerEmoji } from "./Composer";

let restoreLayout: () => void;
beforeAll(() => {
  restoreLayout = installEmojiLayoutStub();
});
afterAll(() => restoreLayout());

const BOOK = "\u{1F4DA}";
const BULB = "\u{1F4A1}";

/** Поле с настоящим состоянием: значение живёт в родителе, как в приложении. */
function Field({ initial, emoji, viewOnly = false }: { initial: string; emoji?: ComposerEmoji; viewOnly?: boolean }) {
  const [value, setValue] = useState(initial);
  return (
    <StubEmojiData>
      <Composer value={value} onChange={setValue} emoji={emoji} viewOnly={viewOnly} buttons={{ emoji: true }} />
    </StubEmojiData>
  );
}

const picker = (onPicked?: (emoji: string) => void): ComposerEmoji => ({ recent: [BOOK, BULB], language: "ru", onPicked });
const textbox = () => screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement;
const emojiButton = () => screen.getByRole("button", { name: "Эмодзи" });
const open = () => fireEvent.click(emojiButton());
const pick = (emoji: string) => fireEvent.click(within(screen.getByRole("dialog", { name: "Выбор эмодзи" })).getByRole("button", { name: emoji }));

describe("Composer: эмодзи", () => {
  it("с пикером кнопка живая: без «Появится позже», открывает панель; без пикера — по-прежнему нарисована", () => {
    const { unmount } = render(<Field initial="" emoji={picker()} viewOnly />);
    expect(emojiButton()).not.toHaveAttribute("aria-disabled");
    expect(emojiButton()).not.toHaveAccessibleDescription("Появится позже");
    expect(emojiButton()).toHaveAttribute("title", "Эмодзи");
    expect(emojiButton()).toHaveAttribute("aria-haspopup", "dialog");
    expect(emojiButton()).toHaveAttribute("aria-expanded", "false");
    open();
    expect(emojiButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("dialog", { name: "Выбор эмодзи" })).toBeInTheDocument();
    unmount();
    render(<Field initial="" viewOnly />);
    expect(emojiButton()).toHaveAttribute("aria-disabled", "true");
    expect(emojiButton()).toHaveAccessibleDescription("Появится позже");
  });

  it("повторное нажатие на кнопку закрывает панель", () => {
    render(<Field initial="" emoji={picker()} />);
    open();
    fireEvent.pointerDown(emojiButton());
    open();
    expect(screen.queryByRole("dialog", { name: "Выбор эмодзи" })).toBeNull();
    expect(emojiButton()).toHaveAttribute("aria-expanded", "false");
  });

  it("эмодзи вставляется в позицию курсора, каретка встаёт после него, фокус в поле, панель закрыта", () => {
    const onPicked = vi.fn();
    render(<Field initial="Привет мир" emoji={picker(onPicked)} />);
    textbox().setSelectionRange(7, 7);
    open();
    pick(BOOK);
    expect(textbox()).toHaveValue(`Привет ${BOOK}мир`);
    expect(textbox().selectionStart).toBe(7 + BOOK.length);
    expect(textbox().selectionEnd).toBe(7 + BOOK.length);
    expect(textbox()).toHaveFocus();
    expect(screen.queryByRole("dialog", { name: "Выбор эмодзи" })).toBeNull();
    expect(onPicked).toHaveBeenCalledExactlyOnceWith(BOOK);
  });

  it("выделение заменяется эмодзи", () => {
    render(<Field initial="Привет мир" emoji={picker()} />);
    textbox().setSelectionRange(7, 10);
    open();
    pick(BULB);
    expect(textbox()).toHaveValue(`Привет ${BULB}`);
    expect(textbox().selectionStart).toBe(7 + BULB.length);
  });

  it("в пустое поле и в начало текста", () => {
    const { unmount } = render(<Field initial="" emoji={picker()} />);
    open();
    pick(BOOK);
    expect(textbox()).toHaveValue(BOOK);
    unmount();
    render(<Field initial="текст" emoji={picker()} />);
    textbox().setSelectionRange(0, 0);
    open();
    pick(BOOK);
    expect(textbox()).toHaveValue(`${BOOK}текст`);
    expect(textbox().selectionStart).toBe(BOOK.length);
  });

  it("два выбора подряд: второй эмодзи встаёт после первого", () => {
    render(<Field initial="a" emoji={picker()} />);
    textbox().setSelectionRange(1, 1);
    open();
    pick(BOOK);
    open();
    pick(BULB);
    expect(textbox()).toHaveValue(`a${BOOK}${BULB}`);
  });

  it("Esc закрывает панель и возвращает фокус в поле; значение не меняется", () => {
    render(<Field initial="текст" emoji={picker()} />);
    open();
    fireEvent.keyDown(screen.getByRole("searchbox", { name: "Найти эмодзи" }), { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Выбор эмодзи" })).toBeNull();
    expect(textbox()).toHaveFocus();
    expect(textbox()).toHaveValue("текст");
  });

  it("щелчок мимо закрывает панель", () => {
    render(<Field initial="текст" emoji={picker()} />);
    open();
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("dialog", { name: "Выбор эмодзи" })).toBeNull();
  });

  it("английский интерфейс: подписи кнопки и панели", () => {
    render(<Field initial="" emoji={{ ...picker(), language: "en" }} />, { language: "en" });
    fireEvent.click(screen.getByRole("button", { name: "Emoji" }));
    expect(screen.getByRole("dialog", { name: "Emoji picker" })).toBeInTheDocument();
    expect(screen.getByRole("searchbox", { name: "Search emoji" })).toBeInTheDocument();
  });

  it("кнопка не показана, если её нет в buttons", () => {
    render(<Composer value="" onChange={vi.fn()} buttons={{ attach: true }} emoji={picker()} />);
    expect(screen.queryByRole("button", { name: "Эмодзи" })).toBeNull();
  });
});
