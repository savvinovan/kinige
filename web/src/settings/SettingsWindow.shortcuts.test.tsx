import { describe, expect, it } from "vitest";
import { fireEvent, screen, waitFor, within } from "../test/render";
import { openWindow } from "../test/prefsWindow";

const tree = () => screen.getByRole("navigation", { name: "Разделы настроек" });
const press = (target: HTMLElement, init: KeyboardEventInit) => fireEvent.keyDown(target, init);

async function openShortcuts() {
  const api = await openWindow();
  fireEvent.click(within(tree()).getByRole("button", { name: "Сочетания клавиш" }));
  return { api, section: screen.getByRole("region", { name: "Сочетания клавиш" }) };
}

describe("Окно «Настройки»: сочетания клавиш", () => {
  it("страница показывает оба сочетания по умолчанию", async () => {
    const { section } = await openShortcuts();
    expect(within(section).getByText("Окно быстрого захвата")).toBeInTheDocument();
    expect(within(section).getByText("Сохранить буфер во «Входящие»")).toBeInTheDocument();
    expect(within(section).queryByRole("button", { name: /Сбросить/ })).toBeNull();
  });

  it("смена: запись нового сочетания нажатием уходит в Rust, строка обновляется, «Сбросить» появляется", async () => {
    const { api, section } = await openShortcuts();
    fireEvent.click(within(section).getByRole("button", { name: /Изменить сочетание: Окно быстрого захвата/ }));
    await waitFor(() => expect(api.recordingLog).toEqual([true]));
    press(document.activeElement as HTMLElement, { code: "KeyK", ctrlKey: true, altKey: true });
    await waitFor(async () => {
      expect((await api.getSettings()).shortcuts[0]).toMatchObject({ kind: "capture", accelerator: "Ctrl+Alt+KeyK", isDefault: false });
    });
    expect(await within(section).findByRole("button", { name: /Сбросить сочетание: Окно быстрого захвата/ })).toBeInTheDocument();
    await waitFor(() => expect(api.recordingLog.at(-1)).toBe(false));
  });

  it("отказ (занято / совпадает с другим): ошибка под строкой, прежнее сочетание остаётся", async () => {
    const { api, section } = await openShortcuts();
    api.failNextShortcut = "shortcut_taken";
    fireEvent.click(within(section).getByRole("button", { name: /Изменить сочетание: Окно быстрого захвата/ }));
    press(document.activeElement as HTMLElement, { code: "KeyK", ctrlKey: true, altKey: true });
    expect(await within(section).findByText(/занято другой программой или системой/)).toBeInTheDocument();
    expect((await api.getSettings()).shortcuts[0]?.accelerator).toBe("Ctrl+Alt+KeyN");
  });

  it("«Сбросить» возвращает умолчание", async () => {
    const api = await openWindow();
    await api.setShortcut("clipboard", "Ctrl+Alt+KeyB");
    fireEvent.click(within(tree()).getByRole("button", { name: "Сочетания клавиш" }));
    fireEvent.click(await screen.findByRole("button", { name: /Сбросить сочетание: Сохранить буфер во «Входящие»/ }));
    await waitFor(async () => expect((await api.getSettings()).shortcuts[1]?.accelerator).toBe("Alt+Shift+KeyV"));
  });

  it("уход на другую страницу убирает режим записи: страница размонтирована, Rust получает «запись закончилась»", async () => {
    const { api, section } = await openShortcuts();
    fireEvent.click(within(section).getByRole("button", { name: /Изменить сочетание: Окно быстрого захвата/ }));
    await waitFor(() => expect(api.recordingLog).toEqual([true]));
    fireEvent.click(within(tree()).getByRole("button", { name: "Копии" }));
    await waitFor(() => expect(api.recordingLog.at(-1)).toBe(false));
  });

  it("правка сочетания в файле видна на странице без перезапуска (событие «настройки изменились»)", async () => {
    const { api, section } = await openShortcuts();
    await api.setShortcut("capture", "Ctrl+Alt+KeyK");
    await waitFor(() => expect(within(section).getByRole("button", { name: /Сбросить сочетание: Окно быстрого захвата/ })).toBeInTheDocument());
  });
});
