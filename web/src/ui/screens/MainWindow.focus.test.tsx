import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import { fireEvent, render, screen } from "../../test/render";
import { AppRoot } from "./AppRoot";

// The focus ring in the main window (from the 4b acceptance feedback): after a mouse click on the profile and closing the menu,
// focus returns to the trigger button (accessibility), but there is no ring: the input mode is `pointer`. After a key
// press (Tab) the mode is `keyboard` and the ring is drawn. jsdom does not apply `:focus-visible` and CSS, so the input
// mode on `<html>` is checked; the rule that hides the ring based on it is in `ui/focus-ring.test.ts`.

const NORMAL = { needsRecovery: false, backups: [], databaseTooNew: false, corruptionFound: false };
const mode = () => document.documentElement.dataset.input;

async function mainWindow() {
  const api = new MockApi({ now: () => new Date(Date.UTC(2026, 9, 21, 6, 12)), startup: NORMAL });
  render(<AppRoot api={api} initialSettings={DEFAULT_SETTINGS} startup={NORMAL} />);
  await screen.findByRole("textbox", { name: "Новая заметка" });
  return screen.getByRole("button", { name: /^Профиль:/ });
}

describe("Главное окно: кольцо фокуса только после клавиши", () => {
  it("мышь: профиль, «Настройки» в меню: фокус вернулся на кнопку профиля, режим «мышь» (кольца нет)", async () => {
    const trigger = await mainWindow();
    expect(mode()).toBe("pointer");
    fireEvent.pointerDown(trigger);
    fireEvent.mouseDown(trigger);
    fireEvent.click(trigger);
    const settings = screen.getByRole("menuitem", { name: /^Настройки/ });
    fireEvent.pointerDown(settings);
    fireEvent.mouseDown(settings);
    fireEvent.click(settings);
    expect(screen.queryByRole("menu")).toBeNull();
    expect(trigger).toHaveFocus();
    expect(mode()).toBe("pointer");
  });

  it("мышь: меню закрыто щелчком мимо: режим «мышь»", async () => {
    const trigger = await mainWindow();
    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);
    expect(screen.getByRole("menu")).toBeInTheDocument();
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("menu")).toBeNull();
    expect(mode()).toBe("pointer");
  });

  it("клавиатура: Escape закрывает меню, фокус на кнопке профиля, режим «клавиатура» (кольцо есть)", async () => {
    const trigger = await mainWindow();
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.keyDown(screen.getAllByRole("menuitem")[0] as HTMLElement, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    expect(trigger).toHaveFocus();
    expect(mode()).toBe("keyboard");
  });

  it("Tab переводит в режим «клавиатура»: кольцо появляется", async () => {
    await mainWindow();
    fireEvent.keyDown(document.body, { key: "Tab" });
    expect(mode()).toBe("keyboard");
    fireEvent.mouseDown(document.body);
    expect(mode()).toBe("pointer");
  });
});
