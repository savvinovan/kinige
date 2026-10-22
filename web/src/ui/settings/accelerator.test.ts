import { describe, expect, it } from "vitest";
import { acceleratorFromEvent, formatAccelerator, keyLabel, parseAccelerator } from "./accelerator";

const press = (code: string, mods: { ctrl?: boolean; alt?: boolean; shift?: boolean; meta?: boolean } = {}) => ({
  code,
  ctrlKey: mods.ctrl ?? false,
  altKey: mods.alt ?? false,
  shiftKey: mods.shift ?? false,
  metaKey: mods.meta ?? false,
});

describe("formatAccelerator", () => {
  it("умолчания: ⌃ ⌥ N и ⌥ ⇧ V", () => {
    expect(formatAccelerator("Ctrl+Alt+KeyN")).toEqual(["⌃", "⌥", "N"]);
    expect(formatAccelerator("Alt+Space")).toEqual(["⌥", "Space"]);
    expect(formatAccelerator("Alt+Shift+KeyV")).toEqual(["⌥", "⇧", "V"]);
  });

  it("порядок значков всегда ⌃ ⌥ ⇧ ⌘, как бы ни была записана строка", () => {
    expect(formatAccelerator("Ctrl+Alt+Shift+Cmd+KeyK")).toEqual(["⌃", "⌥", "⇧", "⌘", "K"]);
    expect(formatAccelerator("Cmd+Shift+Alt+Ctrl+KeyK")).toEqual(["⌃", "⌥", "⇧", "⌘", "K"]);
    expect(formatAccelerator("Cmd+Digit1")).toEqual(["⌘", "1"]);
  });

  it.each("ABCDEFGHIJKLMNOPQRSTUVWXYZ".split(""))("буква %s", (letter) => {
    expect(formatAccelerator(`Alt+Key${letter}`)).toEqual(["⌥", letter]);
  });

  it.each("0123456789".split(""))("цифра %s", (digit) => {
    expect(formatAccelerator(`Ctrl+Digit${digit}`)).toEqual(["⌃", digit]);
  });

  it.each(Array.from({ length: 12 }, (_, index) => index + 1))("функциональная клавиша F%i", (number) => {
    expect(formatAccelerator(`Cmd+F${number}`)).toEqual(["⌘", `F${number}`]);
  });

  it.each([
    ["Backquote", "`"],
    ["Minus", "-"],
    ["Equal", "="],
    ["BracketLeft", "["],
    ["BracketRight", "]"],
    ["Backslash", "\\"],
    ["Semicolon", ";"],
    ["Quote", "'"],
    ["Comma", ","],
    ["Period", "."],
    ["Slash", "/"],
  ])("пунктуация %s", (code, glyph) => {
    expect(formatAccelerator(`Alt+${code}`)).toEqual(["⌥", glyph]);
  });

  it("неизвестный код показывается как есть, пустая строка — пустой набор", () => {
    expect(formatAccelerator("Alt+Numpad7")).toEqual(["⌥", "Numpad7"]);
    expect(formatAccelerator("")).toEqual([]);
  });
});

describe("parseAccelerator", () => {
  it("модификаторы в каноничном порядке и клавиша", () => {
    expect(parseAccelerator("Shift+Alt+KeyV")).toEqual({ modifiers: ["Alt", "Shift"], key: "V" });
    expect(parseAccelerator("Space")).toEqual({ modifiers: [], key: "Space" });
  });
});

describe("acceleratorFromEvent", () => {
  it("собирает каноничную строку: Ctrl, Alt, Shift, Cmd и код клавиши — именно в этом порядке", () => {
    expect(acceleratorFromEvent(press("Space", { alt: true }))).toBe("Alt+Space");
    expect(acceleratorFromEvent(press("KeyV", { alt: true, shift: true }))).toBe("Alt+Shift+KeyV");
    expect(acceleratorFromEvent(press("KeyK", { ctrl: true, alt: true, shift: true, meta: true }))).toBe("Ctrl+Alt+Shift+Cmd+KeyK");
    expect(acceleratorFromEvent(press("Digit5", { meta: true, ctrl: true }))).toBe("Ctrl+Cmd+Digit5");
  });

  it("раскладка не важна: берётся код физической клавиши, а не символ", () => {
    // На русской раскладке клавиша V даёт «м», но `code` остаётся KeyV.
    expect(acceleratorFromEvent({ ...press("KeyV", { alt: true }), key: "м" } as Parameters<typeof acceleratorFromEvent>[0])).toBe("Alt+KeyV");
  });

  it.each(["ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight"])(
    "одиночный модификатор %s не принимается",
    (code) => {
      expect(acceleratorFromEvent(press(code, { alt: true, shift: true, ctrl: true, meta: true }))).toBeNull();
    },
  );

  it.each(["Escape", "Enter", "Tab", "Backspace", "Delete", "ArrowLeft", "ArrowUp", "Home", "PageDown", "CapsLock", "NumpadEnter", "Numpad1", "IntlBackslash"])(
    "клавиша вне списка %s не принимается даже с модификаторами",
    (code) => {
      expect(acceleratorFromEvent(press(code, { alt: true }))).toBeNull();
      expect(acceleratorFromEvent(press(code, { ctrl: true, meta: true }))).toBeNull();
    },
  );

  it("без ⌃, ⌥ и ⌘ клавиша не принимается: обычный ввод нельзя перехватывать; ⇧ одного мало", () => {
    expect(acceleratorFromEvent(press("KeyV"))).toBeNull();
    expect(acceleratorFromEvent(press("Space"))).toBeNull();
    expect(acceleratorFromEvent(press("KeyV", { shift: true }))).toBeNull();
    expect(acceleratorFromEvent(press("Slash"))).toBeNull();
  });

  it("F1–F12 принимаются и без модификаторов", () => {
    expect(acceleratorFromEvent(press("F5"))).toBe("F5");
    expect(acceleratorFromEvent(press("F12", { shift: true }))).toBe("Shift+F12");
  });

  it("результат разбирается обратно: туда и обратно", () => {
    const accelerator = acceleratorFromEvent(press("Backquote", { alt: true, meta: true }));
    expect(accelerator).toBe("Alt+Cmd+Backquote");
    expect(formatAccelerator(accelerator ?? "")).toEqual(["⌥", "⌘", "`"]);
  });
});

describe("keyLabel", () => {
  it("F13 и дальше, буквы нижнего регистра и пустой код не принимаются", () => {
    expect(keyLabel("F13")).toBeNull();
    expect(keyLabel("F0")).toBeNull();
    expect(keyLabel("Keya")).toBeNull();
    expect(keyLabel("")).toBeNull();
  });
});
