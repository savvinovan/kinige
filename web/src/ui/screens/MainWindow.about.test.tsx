import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// "About" in the profile menu (KINIG-54): a window on top of the main one with the build string from `KinigeApi.appInfo()`.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

async function openAbout(language: "ru" | "en", item: string) {
  const api = new MockApi({ now: NOW, language });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language });
  fireEvent.click(await screen.findByRole("button", { name: /^(Профиль|Profile)/ }));
  const menuItem = screen.getByRole("menuitem", { name: item });
  expect(menuItem).not.toHaveAttribute("aria-disabled");
  fireEvent.click(menuItem);
  return api;
}

describe("О программе", () => {
  it("пункт меню профиля открывает окно с версией сборки; Esc закрывает", async () => {
    await openAbout("ru", "О программе");
    const dialog = await screen.findByRole("dialog", { name: "О программе" });
    expect(within(dialog).getByText("Kinige mock")).toBeInTheDocument();
    expect(within(dialog).getByText(/Личный журнал/)).toBeInTheDocument();
    // The window appears after the `appInfo` reply outside act: the DOM is already there, but the Modal effect (focus on
    // the cross and the Esc subscription on `document`) may not have run yet, and an early Esc was lost (KINIG-48). Focus
    // on the cross is a sign that the subscription is in place.
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Закрыть" })).toHaveFocus());
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("на английском: заголовок About и английская строка", async () => {
    await openAbout("en", "About");
    const dialog = await screen.findByRole("dialog", { name: "About" });
    expect(within(dialog).getByText("Kinige mock")).toBeInTheDocument();
    expect(within(dialog).getByText(/personal journal/)).toBeInTheDocument();
  });

  it("версия не получена: окно открывается, вместо версии прочерк", async () => {
    const api = new MockApi({ now: NOW, language: "ru" });
    api.appInfo = () => Promise.reject(new Error("no connection"));
    render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "ru" });
    fireEvent.click(await screen.findByRole("button", { name: /^Профиль/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: "О программе" }));
    const dialog = await screen.findByRole("dialog", { name: "О программе" });
    expect(within(dialog).getByText("—")).toBeInTheDocument();
  });

  it("крестик закрывает окно", async () => {
    await openAbout("ru", "О программе");
    const dialog = await screen.findByRole("dialog", { name: "О программе" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Закрыть" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});
