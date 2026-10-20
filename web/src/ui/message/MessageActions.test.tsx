import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Message } from "./Message";

const sentAt = new Date(2026, 9, 20, 9, 12);
const toolbar = () => screen.getByRole("toolbar", { name: "Действия с сообщением" });
const buttons = () => [...toolbar().querySelectorAll("button")];

const stubClipboard = (writeText: (text: string) => Promise<void>) => {
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
};

describe("ряд действий под плашкой", () => {
  it("пять кнопок: копировать, редактировать, задача, избранное, ещё; ряд — панель инструментов", () => {
    render(<Message markdown="Купить хлеб" sentAt={sentAt} />);
    expect(buttons().map((b) => b.getAttribute("aria-label"))).toEqual([
      "Копировать",
      "Редактировать",
      "Сделать задачей",
      "В избранное",
      "Ещё",
    ]);
  });

  it("кроме «Копировать» кнопки только для вида: aria-disabled и «Появится позже»", () => {
    render(<Message markdown="Купить хлеб" sentAt={sentAt} />);
    const [copy, ...rest] = buttons() as [HTMLButtonElement, ...HTMLButtonElement[]];
    expect(copy).not.toHaveAttribute("aria-disabled");
    for (const button of rest) {
      expect(button).toHaveAttribute("aria-disabled", "true");
      expect(button).toHaveAccessibleDescription("Появится позже");
    }
  });

  it("ряд не в потоке: он внутри обёртки сообщения и скрыт стилем, а не удалён (доступен с клавиатуры)", () => {
    const { container } = render(<Message markdown="Купить хлеб" sentAt={sentAt} />);
    const wrap = container.querySelector(".message-wrap");
    expect(wrap).toContainElement(toolbar());
    // Статья (плашка) и ряд — соседи: ряд не меняет высоту плашки.
    expect(container.querySelector("article")).not.toContainElement(toolbar());
  });

  it("showActions для историй добавляет класс «показан»", () => {
    const { container } = render(<Message markdown="Купить хлеб" sentAt={sentAt} showActions />);
    expect(container.querySelector(".message-wrap")).toHaveClass("message-wrap--actions");
  });

  it("одна остановка Tab: первая кнопка tabindex 0, остальные -1; стрелки и Home/End ходят по кнопкам", () => {
    render(<Message markdown="Купить хлеб" sentAt={sentAt} />);
    const list = buttons();
    expect(list.map((b) => b.tabIndex)).toEqual([0, -1, -1, -1, -1]);
    list[0]?.focus();
    fireEvent.keyDown(toolbar(), { key: "ArrowRight" });
    expect(list[1]).toHaveFocus();
    expect(buttons().map((b) => b.tabIndex)).toEqual([-1, 0, -1, -1, -1]);
    fireEvent.keyDown(toolbar(), { key: "End" });
    expect(list[4]).toHaveFocus();
    fireEvent.keyDown(toolbar(), { key: "ArrowRight" });
    expect(list[0]).toHaveFocus();
    fireEvent.keyDown(toolbar(), { key: "ArrowLeft" });
    expect(list[4]).toHaveFocus();
    fireEvent.keyDown(toolbar(), { key: "Home" });
    expect(list[0]).toHaveFocus();
  });

  describe("«Копировать»", () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => vi.useRealTimers());

    it("пишет в буфер обмена Markdown-текст сообщения и подтверждает «Скопировано»", async () => {
      const writeText = vi.fn(() => Promise.resolve());
      stubClipboard(writeText);
      render(<Message markdown="**жирный** текст" sentAt={sentAt} />);
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Копировать" }));
      });
      expect(writeText).toHaveBeenCalledWith("**жирный** текст");
      expect(screen.getByRole("button", { name: "Скопировано" })).toBeInTheDocument();
      expect(screen.getByRole("status")).toHaveTextContent("Скопировано");
      await act(async () => {
        vi.advanceTimersByTime(2000);
      });
      expect(screen.getByRole("button", { name: "Копировать" })).toBeInTheDocument();
    });

    it("нет доступа к буферу обмена: честное «Не удалось скопировать»", async () => {
      stubClipboard(() => Promise.reject(new Error("NotAllowedError")));
      render(<Message markdown="текст" sentAt={sentAt} />);
      await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Копировать" }));
      });
      expect(screen.getByRole("button", { name: "Не удалось скопировать" })).toBeInTheDocument();
    });
  });
});
