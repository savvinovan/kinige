import { fireEvent, render, screen, within } from "../../test/render";
import { describe, expect, it, vi } from "vitest";
import { DEMO_CHATS } from "../../api/demoData";
import { INBOX_ID } from "../../api/KinigeApi";
import { Sidebar, toSidebarChat } from "./Sidebar";

const CHATS = DEMO_CHATS.map(toSidebarChat);
// `\s` в JavaScript включает и неразрывный, и узкий неразрывный пробелы, которые ставит Intl.
const plain = (text: string | null) => (text ?? "").replace(/\s/g, " ");

describe("Sidebar", () => {
  it("поле «Поиск» и список чатов с подписью для диктора", () => {
    render(<Sidebar chats={CHATS} selectedId={INBOX_ID} />);
    expect(screen.getByRole("searchbox", { name: "Поиск" })).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Чаты" });
    expect(within(nav).getAllByRole("button")).toHaveLength(CHATS.length);
  });

  it("в приложении поля «Поиск» нет до слайса 4 (мёртвых полей нет)", () => {
    render(<Sidebar chats={CHATS} selectedId={INBOX_ID} showSearch={false} />);
    expect(screen.queryByRole("searchbox")).toBeNull();
  });

  it("у Inbox пустое хранимое имя: показывается перевод («Входящие» / «Inbox»)", () => {
    const { unmount } = render(<Sidebar chats={CHATS} selectedId={INBOX_ID} />);
    expect(screen.getByRole("button", { name: /^Входящие/ })).toBeInTheDocument();
    unmount();
    render(<Sidebar chats={CHATS} selectedId={INBOX_ID} />, { language: "en" });
    expect(screen.getByRole("button", { name: /^Inbox/ })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Chats" })).toBeInTheDocument();
  });

  it("выбранный чат помечен aria-current, остальные — нет", () => {
    render(<Sidebar chats={CHATS} selectedId="books" />);
    expect(screen.getByRole("button", { name: /^Книги/ })).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("button", { name: /^Входящие/ })).not.toHaveAttribute("aria-current");
  });

  it("эмодзи — декор для диктора: имя кнопки начинается с названия чата, без эмодзи", () => {
    render(<Sidebar chats={CHATS} selectedId={INBOX_ID} />);
    expect(screen.getByRole("button", { name: /^Идеи/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /💡/ })).toBeNull();
  });

  it("строки — настоящие кнопки с фокусом; выбор вызывает onSelect", () => {
    const onSelect = vi.fn();
    render(<Sidebar chats={CHATS} selectedId={INBOX_ID} onSelect={onSelect} />);
    const row = screen.getByRole("button", { name: /^Работа/ });
    row.focus();
    expect(row).toHaveFocus();
    fireEvent.click(row);
    expect(onSelect).toHaveBeenCalledWith("work");
  });

  it("счётчик записей справа: коротко для глаза, полностью для диктора и подсказки", () => {
    render(<Sidebar chats={CHATS} selectedId={INBOX_ID} />);
    const recipes = screen.getByRole("button", { name: /^Рецепты/ });
    const count = recipes.querySelector(".chat-row__count");
    expect(plain(count?.textContent ?? "")).toBe(", записей: 12 тыс.");
    expect(plain(count?.lastChild?.textContent ?? "")).toBe("12 тыс.");
    expect(plain(recipes.getAttribute("title"))).toBe("Рецепты, 12 345 записей");
    const books = screen.getByRole("button", { name: /^Книги/ });
    expect(plain(books.textContent)).toContain("1 234");
  });

  it("подпись счётчика для диктора читается по-русски и по-английски", () => {
    const { unmount } = render(<Sidebar chats={CHATS} selectedId={INBOX_ID} />);
    expect(plain(screen.getByRole("button", { name: /^Рецепты/ }).textContent)).toBe("🍳Рецепты, записей: 12 тыс.");
    unmount();
    render(<Sidebar chats={CHATS} selectedId={INBOX_ID} />, { language: "en" });
    // Название чата — данные пользователя и не переводится; подпись счётчика и склонение — переводятся.
    expect(plain(screen.getByRole("button", { name: /^Рецепты/ }).textContent)).toBe("🍳Рецепты, entries: 12K");
    expect(screen.getByRole("button", { name: /^Рецепты/ })).toHaveAttribute("title", "Рецепты, 12,345 entries");
  });

  it("склонение в подсказке: 1 запись, 2 записи, 5 записей", () => {
    const chats = [1, 2, 5, 21, 22, 25, 111].map((count) => ({ id: `c${count}`, name: `Чат ${count}`, emoji: "🫙", count }));
    render(<Sidebar chats={chats} selectedId="c1" />);
    const titles = chats.map((c) => screen.getByRole("button", { name: new RegExp(`^Чат ${c.count}$|^Чат ${c.count},`) }).getAttribute("title"));
    expect(titles).toEqual([
      "Чат 1, 1 запись",
      "Чат 2, 2 записи",
      "Чат 5, 5 записей",
      "Чат 21, 21 запись",
      "Чат 22, 22 записи",
      "Чат 25, 25 записей",
      "Чат 111, 111 записей",
    ]);
  });

  it("счётчик «Входящих» равен числу записей в демо-ленте", () => {
    render(<Sidebar chats={CHATS} selectedId={INBOX_ID} />);
    expect(screen.getByRole("button", { name: /^Входящие/ })).toHaveTextContent("9");
  });

  it("ноль и отсутствие числа: счётчика нет", () => {
    render(
      <Sidebar
        chats={[
          { id: "a", name: "Пустой", emoji: "🫙", count: 0 },
          { id: "b", name: "Без числа", emoji: "🫙" },
        ]}
        selectedId="a"
      />,
    );
    expect(screen.getByRole("button", { name: "Пустой" })).not.toHaveTextContent(/\d/);
    expect(screen.getByRole("button", { name: "Без числа" })).not.toHaveTextContent(/\d/);
    expect(document.querySelector(".chat-row__count")).toBeNull();
  });

  it("полное имя длинного чата — в подсказке (в строке оно обрезается многоточием)", () => {
    render(<Sidebar chats={[{ id: "x", name: "Очень длинное название чата про всё на свете", emoji: "🧭", count: 5 }]} selectedId="x" />);
    expect(screen.getByRole("button", { name: /Очень длинное/ })).toHaveAttribute(
      "title",
      "Очень длинное название чата про всё на свете, 5 записей",
    );
  });
});
