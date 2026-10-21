import { act, fireEvent, render, screen } from "../../test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { UNDO_TOAST_MS, UndoToast } from "./UndoToast";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const advance = (ms: number) => act(() => vi.advanceTimersByTime(ms));

const setup = (open = true) => {
  const onUndo = vi.fn();
  const onExpire = vi.fn();
  const view = render(<UndoToast open={open} message="Сообщение удалено" onUndo={onUndo} onExpire={onExpire} />);
  return { onUndo, onExpire, ...view };
};

describe("UndoToast", () => {
  it("срок — 5 секунд", () => {
    expect(UNDO_TOAST_MS).toBe(5000);
  });

  it("область для диктора есть всегда, даже когда тоста нет", () => {
    setup(false);
    const region = screen.getByRole("status");
    expect(region).toHaveAttribute("aria-live", "polite");
    expect(region).toBeEmptyDOMElement();
  });

  it("показывает текст и кнопку «Отменить» — настоящую кнопку, доступную с клавиатуры", () => {
    setup();
    expect(screen.getByRole("status")).toHaveTextContent("Сообщение удалено");
    const undo = screen.getByRole("button", { name: "Отменить" });
    undo.focus();
    expect(undo).toHaveFocus();
  });

  it("«Отменить» вызывает onUndo", () => {
    const { onUndo, onExpire } = setup();
    fireEvent.click(screen.getByRole("button", { name: "Отменить" }));
    expect(onUndo).toHaveBeenCalledTimes(1);
    expect(onExpire).not.toHaveBeenCalled();
  });

  it("через 5 секунд время выходит, раньше — нет", () => {
    const { onExpire } = setup();
    advance(4999);
    expect(onExpire).not.toHaveBeenCalled();
    advance(1);
    expect(onExpire).toHaveBeenCalledTimes(1);
  });

  it("пока на тосте мышь, таймер стоит; после ухода доигрывает остаток", () => {
    const { onExpire } = setup();
    advance(3000);
    fireEvent.mouseEnter(screen.getByRole("button", { name: "Отменить" }).parentElement as HTMLElement);
    advance(60_000);
    expect(onExpire).not.toHaveBeenCalled();
    fireEvent.mouseLeave(screen.getByRole("button", { name: "Отменить" }).parentElement as HTMLElement);
    advance(1999);
    expect(onExpire).not.toHaveBeenCalled();
    advance(1);
    expect(onExpire).toHaveBeenCalledTimes(1);
  });

  it("пока фокус на кнопке, таймер тоже стоит", () => {
    const { onExpire } = setup();
    const undo = screen.getByRole("button", { name: "Отменить" });
    advance(4000);
    act(() => undo.focus());
    advance(30_000);
    expect(onExpire).not.toHaveBeenCalled();
    act(() => undo.blur());
    advance(1000);
    expect(onExpire).toHaveBeenCalledTimes(1);
  });

  it("закрытый тост таймер не запускает", () => {
    const { onExpire } = setup(false);
    advance(10_000);
    expect(onExpire).not.toHaveBeenCalled();
  });
});
