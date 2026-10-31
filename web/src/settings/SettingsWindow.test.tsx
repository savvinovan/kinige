import { afterEach, describe, expect, it, vi } from "vitest";
import { MOCK_SETTINGS_FILE } from "../api/MockApiSlice4b";
import { act, fireEvent, screen, waitFor, within } from "../test/render";
import { openWindow } from "../test/prefsWindow";

afterEach(() => {
  vi.useRealTimers();
  document.documentElement.removeAttribute("data-text-scale");
});

const tree = () => screen.getByRole("navigation", { name: "Разделы настроек" });
const current = () => within(tree()).getByRole("button", { current: "page" });

describe("Окно «Настройки»: дерево разделов", () => {
  it("слева дерево со всеми разделами и вложенными пунктами; выбранный пункт отмечен aria-current", async () => {
    await openWindow();
    const names = within(tree())
      .getAllByRole("button")
      .map((button) => button.textContent);
    // Группы по умолчанию свёрнуты: вложенных пунктов не видно, пока не открыт «Доступ ИИ-агентов» или не нажат шеврон.
    expect(names).toEqual(["Оформление", "Сочетания клавиш", "Копии", "Доступ ИИ-агентов", "Файл настроек"]);
    fireEvent.click(within(tree()).getByRole("button", { name: "Доступ ИИ-агентов" }));
    expect(
      within(tree())
        .getAllByRole("button")
        .map((button) => button.textContent),
    ).toEqual(["Оформление", "Сочетания клавиш", "Копии", "Доступ ИИ-агентов", "Видимые чаты", "Журнал обращений", "Файл настроек"]);
    fireEvent.click(within(tree()).getByRole("button", { name: "Оформление" }));
    expect(current()).toHaveTextContent("Оформление");
    expect(within(tree()).getAllByRole("button", { current: "page" })).toHaveLength(1);
    // Содержимое справа — страница выбранного раздела.
    expect(screen.getByRole("region", { name: "Оформление" })).toBeInTheDocument();

    fireEvent.click(within(tree()).getByRole("button", { name: "Сочетания клавиш" }));
    expect(current()).toHaveTextContent("Сочетания клавиш");
    expect(screen.getByRole("region", { name: "Сочетания клавиш" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Оформление" })).toBeNull();
  });

  it("в обходе Tab один пункт — выбранный; ↑ и ↓ переносят фокус, Home и End — к краям", async () => {
    await openWindow();
    const buttons = within(tree()).getAllByRole("button");
    expect(buttons.filter((button) => button.tabIndex === 0)).toEqual([current()]);
    current().focus();
    fireEvent.keyDown(current(), { key: "ArrowDown" });
    expect(buttons[1]).toHaveFocus();
    fireEvent.keyDown(buttons[1]!, { key: "ArrowDown" });
    expect(buttons[2]).toHaveFocus();
    fireEvent.keyDown(buttons[2]!, { key: "ArrowUp" });
    expect(buttons[1]).toHaveFocus();
    fireEvent.keyDown(buttons[1]!, { key: "End" });
    expect(buttons[buttons.length - 1]).toHaveFocus();
    fireEvent.keyDown(buttons[buttons.length - 1]!, { key: "ArrowDown" });
    expect(buttons[buttons.length - 1]).toHaveFocus();
    fireEvent.keyDown(buttons[buttons.length - 1]!, { key: "Home" });
    expect(buttons[0]).toHaveFocus();
    // Enter на пункте — обычное нажатие кнопки: страница открывается, выбранный пункт меняется.
    buttons[3]!.focus();
    fireEvent.click(buttons[3]!);
    expect(current()).toHaveTextContent("Доступ ИИ-агентов");
  });

  it("подэкран открывается строкой «Настроить ›», в дереве подсвечен вложенный пункт, заголовок со стрелкой назад", async () => {
    await openWindow();
    fireEvent.click(within(tree()).getByRole("button", { name: "Доступ ИИ-агентов" }));
    fireEvent.click(await screen.findByRole("button", { name: /Видимые чаты.*Настроить/ }));
    expect(current()).toHaveTextContent("Видимые чаты");
    const page = screen.getByRole("region", { name: "Видимые чаты" });
    expect(within(page).getByRole("heading", { level: 2, name: "Видимые чаты" })).toBeInTheDocument();
    expect(within(page).getByText("Доступ ИИ-агентов")).toBeInTheDocument();
    fireEvent.click(within(page).getByRole("button", { name: "Назад: Доступ ИИ-агентов" }));
    expect(current()).toHaveTextContent("Доступ ИИ-агентов");
    expect(screen.getByRole("region", { name: "Доступ ИИ-агентов" })).toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: /Журнал обращений.*Настроить/ }));
    expect(current()).toHaveTextContent("Журнал обращений");
  });

  it("у пункта «Файл настроек» метка ошибки, пока файл не разобран, небезопасен или в нём есть ошибки ключей", async () => {
    const api = await openWindow();
    expect(within(tree()).queryByRole("button", { name: /есть проблемы/ })).toBeNull();
    api.settingsFile = { ...MOCK_SETTINGS_FILE, problem: { kind: "parse", line: 1, column: 15 } };
    act(() => api.emitSettingsChanged());
    expect(await within(tree()).findByRole("button", { name: "Файл настроек, есть проблемы" })).toBeInTheDocument();
    api.settingsFile = { ...MOCK_SETTINGS_FILE, issues: [{ key: "text_size", kind: "value" }] };
    act(() => api.emitSettingsChanged());
    await within(tree()).findByRole("button", { name: "Файл настроек, есть проблемы" });
    api.settingsFile = MOCK_SETTINGS_FILE;
    act(() => api.emitSettingsChanged());
    await waitFor(() => expect(within(tree()).queryByRole("button", { name: /есть проблемы/ })).toBeNull());
  });
});

describe("Окно «Настройки»: прокрутка страницы", () => {
  // Один прокручиваемый блок на все страницы: переход (дерево, «Настроить ›», «назад») открывает страницу сверху.
  const scroller = () => document.querySelector<HTMLElement>(".prefs__page") as HTMLElement;
  const scrollTo300 = () => {
    // jsdom не считает вёрстку: положение прокрутки хранится в свойстве, как у настоящего элемента.
    let top = 300;
    Object.defineProperty(scroller(), "scrollTop", { configurable: true, get: () => top, set: (value: number) => (top = value) });
  };

  it("дерево: страница, на которую перешли, открывается сверху", async () => {
    await openWindow();
    fireEvent.click(within(tree()).getByRole("button", { name: "Доступ ИИ-агентов" }));
    scrollTo300();
    expect(scroller().scrollTop).toBe(300);
    fireEvent.click(within(tree()).getByRole("button", { name: "Копии" }));
    expect(scroller().scrollTop).toBe(0);
  });

  it("«Настроить ›» и «назад»: подэкран и возврат тоже сверху", async () => {
    await openWindow();
    fireEvent.click(within(tree()).getByRole("button", { name: "Доступ ИИ-агентов" }));
    scrollTo300();
    fireEvent.click(await screen.findByRole("button", { name: /Журнал обращений.*Настроить/ }));
    expect(scroller().scrollTop).toBe(0);
    scrollTo300();
    fireEvent.click(screen.getByRole("button", { name: "Назад: Доступ ИИ-агентов" }));
    expect(scroller().scrollTop).toBe(0);
  });

  it("правка данных на странице (событие «настройки изменились») прокрутку не сбрасывает", async () => {
    const api = await openWindow();
    fireEvent.click(within(tree()).getByRole("button", { name: "Доступ ИИ-агентов" }));
    scrollTo300();
    act(() => api.emitSettingsChanged());
    await waitFor(() => expect(api.calls.filter((call) => call.startsWith("getSettingsFile")).length).toBeGreaterThan(1));
    expect(scroller().scrollTop).toBe(300);
  });
});

describe("Окно «Настройки»: оформление и копии", () => {
  it("«Размер текста» → «Крупный»: атрибут на <html> сразу и сохранён в настройках", async () => {
    const api = await openWindow();
    expect(document.documentElement).not.toHaveAttribute("data-text-scale");
    fireEvent.click(screen.getByRole("radio", { name: "Крупный" }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-text-scale", "large"));
    expect((await api.getSettings()).textSize).toBe("large");
    fireEvent.click(screen.getByRole("radio", { name: "Обычный" }));
    await waitFor(() => expect(document.documentElement).not.toHaveAttribute("data-text-scale"));
  });

  it("отказ сохранить размер текста: возвращается прежний, ошибка не падает", async () => {
    const api = await openWindow();
    api.failNext("setTextSize", "store_write_failed");
    fireEvent.click(screen.getByRole("radio", { name: "Крупный" }));
    await waitFor(() => expect(document.documentElement).not.toHaveAttribute("data-text-scale"));
    expect((await api.getSettings()).textSize).toBe("normal");
  });

  it("раздел «Копии» показывает папку и предупреждение «открытый текст»; «Выбрать…» меняет папку", async () => {
    const api = await openWindow({ pickedFolder: "/Volumes/Backup/Журнал" });
    fireEvent.click(within(tree()).getByRole("button", { name: "Копии" }));
    expect(screen.getByText("/Users/me/Documents/Kinige")).toBeInTheDocument();
    expect(screen.getByText(/открытый текст/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Выбрать…" }));
    expect(await screen.findByText("/Volumes/Backup/Журнал")).toBeInTheDocument();
    expect((await api.getSettings()).mirror.folder).toBe("/Volumes/Backup/Журнал");
  });

  it("отмена системного диалога оставляет папку; ошибка выбора видна", async () => {
    const api = await openWindow({ pickedFolder: null });
    fireEvent.click(within(tree()).getByRole("button", { name: "Копии" }));
    fireEvent.click(screen.getByRole("button", { name: "Выбрать…" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Выбрать…" })).not.toHaveAttribute("aria-disabled", "true"));
    expect(screen.getByText("/Users/me/Documents/Kinige")).toBeInTheDocument();
    api.failNextPick = "folder_pick_failed";
    fireEvent.click(screen.getByRole("button", { name: "Выбрать…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Не удалось выбрать папку.");
  });

  it("окно открывается со свежим состоянием копии и перечитывает его само: нет ложного «Копия актуальна»", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const api = await openWindow();
    fireEvent.click(within(tree()).getByRole("button", { name: "Копии" }));
    expect(screen.getByText("Копия актуальна")).toBeInTheDocument();
    // Папка стала недоступна уже при открытом окне.
    const settings = await api.getSettings();
    api.getSettings = () => Promise.resolve({ ...settings, mirror: { ...settings.mirror, error: "folder_unavailable", pendingDays: 2 } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2100);
    });
    expect(await screen.findByText(/Папка недоступна/)).toBeInTheDocument();
    expect(screen.queryByText("Копия актуальна")).toBeNull();
  });

  it("«Изменено через settings.json · время» стоит рядом с разделом, который поменял файл", async () => {
    const api = await openWindow();
    api.settingsFile = {
      ...MOCK_SETTINGS_FILE,
      changed: { appearance: Date.UTC(2026, 9, 28, 11, 2), shortcuts: null, mirror: Date.UTC(2026, 9, 28, 11, 5), access: null },
    };
    act(() => api.emitSettingsChanged());
    expect(await screen.findByText(/Изменено через settings\.json · .*\d{2}:\d{2}/)).toBeInTheDocument();
    expect(screen.getAllByText(/Изменено через settings\.json/)).toHaveLength(1);
    fireEvent.click(within(tree()).getByRole("button", { name: "Копии" }));
    expect(screen.getAllByText(/Изменено через settings\.json/)).toHaveLength(1);
    fireEvent.click(within(tree()).getByRole("button", { name: "Сочетания клавиш" }));
    expect(screen.queryByText(/Изменено через settings\.json/)).toBeNull();
  });

  it("событие «настройки изменились» перечитывает размер текста: правка файла видна без перезапуска", async () => {
    const api = await openWindow();
    await api.setTextSize("large");
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-text-scale", "large"));
    expect(screen.getByRole("radio", { name: "Крупный" })).toBeChecked();
  });
});

describe("Окно «Настройки»: английский", () => {
  it("те же страницы и подписи на английском", async () => {
    await openWindow({}, "en");
    const names = within(screen.getByRole("navigation", { name: "Settings sections" }))
      .getAllByRole("button")
      .map((button) => button.textContent);
    // Группа по умолчанию свёрнута; раскрывается шевроном.
    expect(names).toEqual(["Appearance", "Keyboard shortcuts", "Copies", "AI agent access", "Settings file"]);
    fireEvent.click(screen.getByTestId("tree-chevron-access"));
    expect(screen.getByRole("button", { name: "Visible chats" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Access log" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Large" })).toBeInTheDocument();
  });
});
