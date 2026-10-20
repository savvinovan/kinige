import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DEMO_CHATS } from "../../api/demoData";
import { Sidebar } from "./Sidebar";

describe("Sidebar", () => {
  it("поле «Поиск» и список чатов с подписью для диктора", () => {
    render(<Sidebar chats={DEMO_CHATS} selectedId="inbox" />);
    expect(screen.getByRole("searchbox", { name: "Поиск" })).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Чаты" });
    expect(within(nav).getAllByRole("button")).toHaveLength(DEMO_CHATS.length);
  });

  it("выбранный чат помечен aria-current, остальные — нет", () => {
    render(<Sidebar chats={DEMO_CHATS} selectedId="books" />);
    expect(screen.getByRole("button", { name: /^Книги/ })).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("button", { name: /^Входящие/ })).not.toHaveAttribute("aria-current");
  });

  it("эмодзи — декор для диктора: имя кнопки начинается с названия чата, без эмодзи", () => {
    render(<Sidebar chats={DEMO_CHATS} selectedId="inbox" />);
    expect(screen.getByRole("button", { name: /^Идеи/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /💡/ })).toBeNull();
  });

  it("строки — настоящие кнопки с фокусом; выбор вызывает onSelect", () => {
    const onSelect = vi.fn();
    render(<Sidebar chats={DEMO_CHATS} selectedId="inbox" onSelect={onSelect} />);
    const row = screen.getByRole("button", { name: /^Работа/ });
    row.focus();
    expect(row).toHaveFocus();
    fireEvent.click(row);
    expect(onSelect).toHaveBeenCalledWith("work");
  });

  it("счётчик записей справа: коротко для глаза, полностью для диктора и подсказки", () => {
    render(<Sidebar chats={DEMO_CHATS} selectedId="inbox" />);
    const recipes = screen.getByRole("button", { name: /^Рецепты/ });
    expect(within(recipes).getByText("12 тыс.")).toHaveClass("chat-row__count", { exact: false });
    expect(recipes).toHaveAccessibleName("Рецепты записей: 12 тыс.");
    expect(recipes).toHaveAttribute("title", "Рецепты, 12 345 записей");
    const books = screen.getByRole("button", { name: /^Книги/ });
    // jest-dom сводит все пробелы к обычному, поэтому узкий неразрывный проверяем по сырому textContent.
    expect(books.textContent).toContain("1 234");
  });

  it("счётчик «Входящих» равен числу записей в демо-ленте", () => {
    render(<Sidebar chats={DEMO_CHATS} selectedId="inbox" />);
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
