import { describe, expect, it } from "vitest";
import { fireEvent, render } from "../test/render";
import { useInputModality } from "./useInputModality";

function Probe() {
  useInputModality();
  return <button type="button">кнопка</button>;
}

const mode = () => document.documentElement.dataset.input;

describe("Откуда пришёл ввод (кольцо фокуса только с клавиатуры)", () => {
  it("сначала — мышь (колец нет); клавиша переводит в клавиатуру, нажатие мыши возвращает", () => {
    const { unmount } = render(<Probe />);
    expect(mode()).toBe("pointer");
    fireEvent.keyDown(document.body, { key: "ArrowDown" });
    expect(mode()).toBe("keyboard");
    fireEvent.mouseDown(document.body);
    expect(mode()).toBe("pointer");
    fireEvent.keyDown(document.body, { key: "Tab" });
    expect(mode()).toBe("keyboard");
    unmount();
    expect(mode()).toBeUndefined();
  });

  it("одни модификаторы (⌘, ⌥, ⌃, ⇧) режим не меняют", () => {
    render(<Probe />);
    for (const key of ["Meta", "Alt", "Control", "Shift"]) fireEvent.keyDown(document.body, { key });
    expect(mode()).toBe("pointer");
  });
});
