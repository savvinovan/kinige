import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import { fireEvent, render, screen, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// «О программе» в меню профиля (KINIG-54): окно поверх главного со строкой сборки от `KinigeApi.appInfo()`.
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
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("на английском: заголовок About и английская строка", async () => {
    await openAbout("en", "About");
    const dialog = await screen.findByRole("dialog", { name: "About" });
    expect(within(dialog).getByText("Kinige mock")).toBeInTheDocument();
    expect(within(dialog).getByText(/personal journal/)).toBeInTheDocument();
  });

  it("крестик закрывает окно", async () => {
    await openAbout("ru", "О программе");
    const dialog = await screen.findByRole("dialog", { name: "О программе" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Закрыть" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
