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
    expect(screen.getByRole("button", { name: "Книги" })).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("button", { name: "Входящие" })).not.toHaveAttribute("aria-current");
  });

  it("эмодзи — декор для диктора: имя кнопки — только название чата", () => {
    render(<Sidebar chats={DEMO_CHATS} selectedId="inbox" />);
    expect(screen.getByRole("button", { name: "Идеи" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /💡/ })).toBeNull();
  });

  it("строки — настоящие кнопки с фокусом; выбор вызывает onSelect", () => {
    const onSelect = vi.fn();
    render(<Sidebar chats={DEMO_CHATS} selectedId="inbox" onSelect={onSelect} />);
    const row = screen.getByRole("button", { name: "Работа" });
    row.focus();
    expect(row).toHaveFocus();
    fireEvent.click(row);
    expect(onSelect).toHaveBeenCalledWith("work");
  });

  it("полное имя длинного чата — в подсказке (в строке оно обрезается)", () => {
    render(<Sidebar chats={[{ id: "x", name: "Очень длинное название чата про всё на свете", emoji: "🧭" }]} selectedId="x" />);
    expect(screen.getByRole("button", { name: /Очень длинное/ })).toHaveAttribute("title", "Очень длинное название чата про всё на свете");
  });
});
