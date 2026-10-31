import { describe, expect, it, vi } from "vitest";
import type { ShortcutStatus } from "../../api/generated/ShortcutStatus";
import { act, fireEvent, render, screen } from "../../test/render";
import { ShortcutRow } from "./ShortcutRow";

const status = (patch: Partial<ShortcutStatus> = {}): ShortcutStatus => ({
  kind: "capture",
  accelerator: "Alt+Space",
  isDefault: true,
  state: "assigned",
  ...patch,
});

function setup(patch: Partial<ShortcutStatus> = {}, props: { error?: string } = {}, language: "ru" | "en" = "ru") {
  const onChange = vi.fn();
  const onReset = vi.fn();
  const onRecording = vi.fn();
  const view = render(<ShortcutRow status={status(patch)} onChange={onChange} onReset={onReset} onRecording={onRecording} {...props} />, { language });
  return { onChange, onReset, onRecording, ...view };
}

const changeButton = () => screen.getByRole("button", { name: /^Изменить/ });
const startRecording = () => fireEvent.click(changeButton());
const recorder = () => screen.getByRole("group", { name: "Новое сочетание: Окно быстрого захвата" });
const press = (code: string, init: KeyboardEventInit = {}) => fireEvent.keyDown(document.activeElement!, { code, key: code, ...init });

describe("ShortcutRow: вид", () => {
  it("название, капсулы с подписью для диктора, кнопка «Изменить»; «Сбросить» нет у сочетания по умолчанию", () => {
    const { container } = setup();
    expect(screen.getByRole("group", { name: "Окно быстрого захвата" })).toBeInTheDocument();
    expect([...container.querySelectorAll("kbd")].map((cap) => cap.textContent)).toEqual(["⌥", "Space"]);
    expect(container.querySelectorAll("kbd[aria-hidden='true']")).toHaveLength(2);
    expect(screen.getByRole("img", { name: "Сочетание: Option + Space" })).toBeInTheDocument();
    expect(changeButton()).toHaveAccessibleName("Изменить сочетание: Окно быстрого захвата");
    expect(changeButton()).toHaveTextContent("Изменить");
    expect(screen.queryByRole("button", { name: /Сбросить/ })).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("назначено — ничего лишнего: ни строки состояния, ни ошибки", () => {
    const { container } = setup();
    expect(container.querySelector(".status-note")).toBeNull();
    expect(container.querySelector(".shortcut-row__keys--taken")).toBeNull();
  });

  it("своё сочетание: «Сбросить» вызывает onReset", () => {
    const { onReset } = setup({ accelerator: "Ctrl+Alt+Shift+Cmd+KeyK", isDefault: false });
    expect(screen.getByRole("img", { name: "Сочетание: Control + Option + Shift + Command + K" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Сбросить сочетание: Окно быстрого захвата" }));
    expect(onReset).toHaveBeenCalledTimes(1);
  });

  it("занято: строка ошибки со значком, капсулы приглушены, «Сбросить» есть даже у умолчания", () => {
    const { container } = setup({ state: "taken" });
    expect(screen.getByText("Не назначено: занято другой программой или системой").closest("[role='status']")).not.toBeNull();
    expect(container.querySelector(".status-note--error .icon")).not.toBeNull();
    expect(container.querySelector(".shortcut-row__keys--taken")).not.toBeNull();
    expect(screen.getByRole("button", { name: /Сбросить/ })).toBeInTheDocument();
  });

  it("ошибка сохранения заменяет «занято» и объявляется как alert", () => {
    setup({ state: "taken" }, { error: "Это сочетание занято другой программой или системой." });
    expect(screen.getByRole("alert")).toHaveTextContent("Это сочетание занято другой программой или системой.");
    expect(screen.queryByText("Не назначено: занято другой программой или системой")).toBeNull();
  });

  it("английский: подписи", () => {
    setup({ kind: "clipboard", accelerator: "Alt+Shift+KeyV", isDefault: false, state: "taken" }, {}, "en");
    expect(screen.getByRole("group", { name: "Save clipboard to Inbox" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Shortcut: Option + Shift + V" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Change shortcut: Save clipboard to Inbox" })).toHaveTextContent("Change");
    expect(screen.getByRole("button", { name: "Reset shortcut: Save clipboard to Inbox" })).toHaveTextContent("Reset");
    expect(screen.getByText("Not assigned: already used by another app or the system")).toBeInTheDocument();
  });
});

describe("ShortcutRow: запись нового сочетания", () => {
  it("«Изменить» включает запись: подсказка, фокус в поле записи, onRecording(true), объявление диктору", () => {
    const { onRecording, container } = setup();
    const live = container.querySelector("[aria-live='polite']");
    expect(live).toBeEmptyDOMElement();
    startRecording();
    expect(recorder()).toHaveFocus();
    // В поле записи — короткая подсказка (раскладка строки не прыгает), полная — во всплывающей подсказке и в объявлении диктору.
    expect(recorder()).toHaveTextContent("Нажмите клавиши…");
    expect(recorder()).toHaveAttribute("title", "Нажмите новое сочетание с ⌃, ⌥ или ⌘… Esc — отмена");
    expect(onRecording).toHaveBeenCalledTimes(1);
    expect(onRecording).toHaveBeenLastCalledWith(true);
    // Область status есть всегда, текст в ней появился с началом записи.
    expect(live).toHaveTextContent("Нажмите новое сочетание");
    expect(screen.queryByRole("img", { name: /Сочетание:/ })).toBeNull();
  });

  it("сочетание принимается: onChange с каноничной строкой, запись заканчивается, фокус возвращается на «Изменить»", () => {
    const { onChange, onRecording } = setup();
    startRecording();
    press("KeyV", { altKey: true, shiftKey: true });
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("Alt+Shift+KeyV");
    expect(onRecording).toHaveBeenLastCalledWith(false);
    expect(screen.queryByRole("group", { name: /Новое сочетание/ })).toBeNull();
    expect(changeButton()).toHaveFocus();
  });

  it("порядок в строке — Ctrl, Alt, Shift, Cmd, какие бы модификаторы ни были нажаты", () => {
    const { onChange } = setup();
    startRecording();
    press("Digit7", { metaKey: true, shiftKey: true, altKey: true, ctrlKey: true });
    expect(onChange).toHaveBeenCalledWith("Ctrl+Alt+Shift+Cmd+Digit7");
  });

  it("одиночные модификаторы, Enter, Tab, стрелки и буква без модификаторов игнорируются: запись продолжается", () => {
    const { onChange, onRecording } = setup();
    startRecording();
    press("AltLeft", { altKey: true });
    press("ShiftLeft", { shiftKey: true, altKey: true });
    press("Enter");
    press("Enter", { altKey: true });
    press("Tab");
    press("ArrowLeft", { metaKey: true });
    press("KeyV");
    press("KeyV", { shiftKey: true });
    expect(onChange).not.toHaveBeenCalled();
    expect(recorder()).toHaveFocus();
    expect(onRecording).toHaveBeenCalledTimes(1);
  });

  it("в записи нажатие не уходит дальше: действие браузера отменено, окно настроек (Tab, Esc) его не видит", () => {
    const outside = vi.fn();
    document.addEventListener("keydown", outside);
    setup();
    startRecording();
    expect(fireEvent.keyDown(document.activeElement!, { code: "Tab", key: "Tab" })).toBe(false);
    expect(fireEvent.keyDown(document.activeElement!, { code: "Space", key: " ", altKey: true })).toBe(false);
    expect(outside).not.toHaveBeenCalled();
    document.removeEventListener("keydown", outside);
  });

  it("Esc отменяет запись: onChange нет, onRecording(false), фокус на «Изменить»", () => {
    const { onChange, onRecording } = setup();
    startRecording();
    press("Escape");
    expect(onChange).not.toHaveBeenCalled();
    expect(onRecording).toHaveBeenLastCalledWith(false);
    expect(changeButton()).toHaveFocus();
  });

  it("Option+Esc не отмена и не сочетание: запись продолжается", () => {
    const { onChange } = setup();
    startRecording();
    press("Escape", { altKey: true });
    expect(onChange).not.toHaveBeenCalled();
    expect(recorder()).toHaveFocus();
  });

  it("потеря фокуса (щелчок в другое место) отменяет запись, фокус не отбирается", () => {
    const { onChange, onRecording } = setup();
    startRecording();
    const other = document.createElement("button");
    document.body.append(other);
    act(() => other.focus());
    expect(screen.queryByRole("group", { name: /Новое сочетание/ })).toBeNull();
    expect(onRecording).toHaveBeenLastCalledWith(false);
    expect(onChange).not.toHaveBeenCalled();
    expect(other).toHaveFocus();
    other.remove();
  });

  it("размонтирование во время записи сообщает onRecording(false): Rust вернёт глобальные сочетания", () => {
    const { onRecording, unmount } = setup();
    startRecording();
    unmount();
    expect(onRecording).toHaveBeenLastCalledWith(false);
  });

  it("IME: нажатие во время композиции не принимается", () => {
    const { onChange } = setup();
    startRecording();
    press("KeyV", { altKey: true, isComposing: true });
    expect(onChange).not.toHaveBeenCalled();
  });

  it("кнопки на время записи недоступны с клавиатуры и мыши (скрыты), после записи возвращаются", () => {
    const { container } = setup({ isDefault: false });
    startRecording();
    expect(container.querySelector(".shortcut-row__actions--hidden")).not.toBeNull();
    press("Escape");
    expect(container.querySelector(".shortcut-row__actions--hidden")).toBeNull();
    expect(screen.getByRole("button", { name: /Сбросить/ })).toBeInTheDocument();
  });

  it("английский: подсказка записи", () => {
    setup({}, {}, "en");
    fireEvent.click(screen.getByRole("button", { name: "Change shortcut: Quick capture window" }));
    const recording = screen.getByRole("group", { name: "New shortcut for Quick capture window" });
    expect(recording).toHaveTextContent("Press keys…");
    expect(recording).toHaveAttribute("title", "Press the new shortcut with ⌃, ⌥ or ⌘… Esc to cancel");
  });
});
