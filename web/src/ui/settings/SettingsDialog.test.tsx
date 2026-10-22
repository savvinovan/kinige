import { describe, expect, it, vi } from "vitest";
import type { MirrorStatus } from "../../api/generated/MirrorStatus";
import type { Settings } from "../../api/generated/Settings";
import { fireEvent, render, screen, within } from "../../test/render";
import { DEFAULT_SHORTCUTS } from "./demoShortcuts";
import { SettingsDialog } from "./SettingsDialog";

const mirror = (patch: Partial<MirrorStatus> = {}): MirrorStatus => ({
  folder: "~/Documents/Kinige",
  isDefault: true,
  error: null,
  pendingDays: 0,
  ...patch,
});
const settings = (patch: Partial<MirrorStatus> = {}, textSize: Settings["textSize"] = "normal"): Settings => ({
  textSize,
  mirror: mirror(patch),
  shortcuts: DEFAULT_SHORTCUTS,
});

const noop = () => {};
const handlers = {
  shortcuts: DEFAULT_SHORTCUTS,
  onShortcutChange: noop,
  onShortcutReset: noop,
  onShortcutRecording: noop,
};
const view = (props: Partial<Parameters<typeof SettingsDialog>[0]> = {}, language: "ru" | "en" = "ru") =>
  render(<SettingsDialog settings={settings()} {...handlers} onClose={noop} onChooseFolder={noop} onTextSizeChange={noop} {...props} />, { language });

describe("SettingsDialog", () => {
  it("модальное окно «Настройки» с тремя разделами", () => {
    view();
    const dialog = screen.getByRole("dialog", { name: "Настройки" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(screen.getAllByRole("region")).toHaveLength(3);
    expect(screen.getByRole("region", { name: "Копия в Markdown" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Сочетания клавиш" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Размер текста" })).toBeInTheDocument();
    expect(screen.getByText(/по файлу на день/)).toBeInTheDocument();
  });

  it("папка по умолчанию и актуальна: путь, бейдж, «Копия актуальна», предупреждение", () => {
    view();
    expect(screen.getByText("~/Documents/Kinige")).toBeInTheDocument();
    expect(screen.getByText("Папка по умолчанию")).toBeInTheDocument();
    expect(screen.getByText("Копия актуальна")).toBeInTheDocument();
    expect(screen.getByText(/Копия — открытый текст/)).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("папка не выбрана: пустое состояние, предупреждение видно, статуса нет", () => {
    view({ settings: settings({ folder: null, isDefault: false }) });
    expect(screen.getByText("Папка не выбрана")).toBeInTheDocument();
    expect(screen.getByText(/Копия — открытый текст/)).toBeInTheDocument();
    expect(screen.queryByText("Копия актуальна")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it.each([
    [1, "1 день ждёт записи в копию"],
    [2, "2 дня ждут записи в копию"],
    [5, "5 дней ждут записи в копию"],
    [21, "21 день ждёт записи в копию"],
  ])("ожидающие дни (ru): %i", (pendingDays, text) => {
    view({ settings: settings({ pendingDays, isDefault: false, folder: "/Users/me/Notes" }) });
    expect(screen.getByRole("status")).toHaveTextContent(text);
    expect(screen.queryByText("Копия актуальна")).toBeNull();
  });

  it.each([
    ["folder_unavailable", /Папка недоступна/],
    ["write_failed", /Не удалось записать копию/],
  ] as const)("ошибка %s: role=alert, статус «актуальна» не показывается", (error, text) => {
    view({ settings: settings({ error, pendingDays: 3 }) });
    expect(screen.getByRole("alert")).toHaveTextContent(text);
    expect(screen.queryByText("Копия актуальна")).toBeNull();
    expect(screen.queryByText(/ждут записи/)).toBeNull();
    // Предупреждение остаётся.
    expect(screen.getByText(/Копия — открытый текст/)).toBeInTheDocument();
  });

  it("ошибка выбора папки (actionError) — отдельный alert под строкой папки", () => {
    view({ actionError: "Не удалось открыть выбор папки." });
    expect(screen.getByRole("alert")).toHaveTextContent("Не удалось открыть выбор папки.");
  });

  it("«Выбрать…» вызывает onChooseFolder; при choosing не вызывает", () => {
    const onChooseFolder = vi.fn();
    const { rerender } = view({ onChooseFolder });
    fireEvent.click(screen.getByRole("button", { name: "Выбрать…" }));
    expect(onChooseFolder).toHaveBeenCalledTimes(1);
    rerender(<SettingsDialog settings={settings()} {...handlers} onClose={noop} onChooseFolder={onChooseFolder} onTextSizeChange={noop} choosing />);
    const button = screen.getByRole("button", { name: "Выбрать…" });
    expect(button).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(button);
    expect(onChooseFolder).toHaveBeenCalledTimes(1);
  });

  it("размер текста: «Обычный» выбран, «Крупный» вызывает onTextSizeChange; стрелка тоже", () => {
    const onTextSizeChange = vi.fn();
    view({ onTextSizeChange });
    const group = screen.getByRole("radiogroup", { name: "Размер текста" });
    expect(within(group).getByRole("radio", { name: "Обычный" })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(within(group).getByRole("radio", { name: "Крупный" }));
    expect(onTextSizeChange).toHaveBeenLastCalledWith("large");
    const normal = within(group).getByRole("radio", { name: "Обычный" });
    fireEvent.keyDown(normal, { key: "ArrowRight" });
    expect(onTextSizeChange).toHaveBeenCalledTimes(2);
    expect(onTextSizeChange).toHaveBeenLastCalledWith("large");
  });

  it("Tab обходит окно по кругу, группа размера текста — одна остановка (найдено в настоящем окне)", () => {
    view();
    const stops: string[] = [];
    const tab = () => {
      fireEvent.keyDown(document.activeElement ?? document.body, { key: "Tab" });
      stops.push(document.activeElement?.getAttribute("aria-label") ?? document.activeElement?.textContent ?? "");
    };
    stops.push(document.activeElement?.getAttribute("aria-label") ?? "");
    for (let i = 0; i < 6; i++) tab();
    expect(stops).toEqual([
      "Закрыть настройки",
      "Выбрать…",
      "Изменить сочетание: Окно быстрого захвата",
      "Изменить сочетание: Сохранить буфер во «Входящие»",
      "Обычный",
      "Закрыть настройки",
      "Выбрать…",
    ]);
  });

  it("«Крупный» выбран по настройкам", () => {
    view({ settings: settings({}, "large") });
    expect(screen.getByRole("radio", { name: "Крупный" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("radio", { name: "Обычный" })).toHaveAttribute("aria-checked", "false");
  });

  it("Esc и кнопка «Закрыть настройки» вызывают onClose; фокус сначала на кнопке закрытия", () => {
    const onClose = vi.fn();
    view({ onClose });
    const close = screen.getByRole("button", { name: "Закрыть настройки" });
    expect(close).toHaveFocus();
    fireEvent.click(close);
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("английский: заголовки, статус, ошибка, множественное число", () => {
    const { unmount } = view({ settings: settings({ pendingDays: 1 }) }, "en");
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Markdown copy" })).toBeInTheDocument();
    expect(screen.getByRole("radiogroup", { name: "Text size" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Large" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choose…" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close settings" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("1 day is waiting to be written");
    expect(screen.getByText(/The copy is plain text/)).toBeInTheDocument();
    unmount();
    view({ settings: settings({ pendingDays: 4 }) }, "en");
    expect(screen.getByRole("status")).toHaveTextContent("4 days are waiting to be written");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("раздел «Сочетания клавиш»: две строки по умолчанию со своими капсулами, «Сбросить» нет", () => {
    view();
    const region = screen.getByRole("region", { name: "Сочетания клавиш" });
    const capture = within(region).getByRole("group", { name: "Окно быстрого захвата" });
    const clipboard = within(region).getByRole("group", { name: "Сохранить буфер во «Входящие»" });
    expect(within(capture).getByRole("img", { name: "Сочетание: Option + Space" })).toBeInTheDocument();
    expect(within(clipboard).getByRole("img", { name: "Сочетание: Option + Shift + V" })).toBeInTheDocument();
    expect(within(region).queryByRole("button", { name: /Сбросить/ })).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("раздел «Сочетания клавиш»: действия приходят в родителя с видом действия", () => {
    const onShortcutChange = vi.fn();
    const onShortcutReset = vi.fn();
    const onShortcutRecording = vi.fn();
    const shortcuts = [
      { kind: "capture", accelerator: "Alt+Space", isDefault: true, state: "assigned" },
      { kind: "clipboard", accelerator: "Ctrl+KeyJ", isDefault: false, state: "assigned" },
    ] as const;
    view({ shortcuts, onShortcutChange, onShortcutReset, onShortcutRecording });
    fireEvent.click(screen.getByRole("button", { name: "Сбросить сочетание: Сохранить буфер во «Входящие»" }));
    expect(onShortcutReset).toHaveBeenCalledWith("clipboard");
    fireEvent.click(screen.getByRole("button", { name: "Изменить сочетание: Окно быстрого захвата" }));
    expect(onShortcutRecording).toHaveBeenLastCalledWith(true);
    fireEvent.keyDown(document.activeElement!, { code: "KeyK", key: "k", altKey: true, metaKey: true });
    expect(onShortcutChange).toHaveBeenCalledWith("capture", "Alt+Cmd+KeyK");
    expect(onShortcutRecording).toHaveBeenLastCalledWith(false);
  });

  it("ошибка сохранения сочетания — role=alert под нужной строкой", () => {
    view({ shortcutErrors: { clipboard: "Это сочетание уже назначено другому действию." } });
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Это сочетание уже назначено другому действию.");
    expect(screen.getByRole("group", { name: "Сохранить буфер во «Входящие»" })).toContainElement(alert);
  });

  it("запись сочетания в окне: Esc выходит из записи и не закрывает окно настроек", () => {
    const onClose = vi.fn();
    view({ onClose });
    fireEvent.click(screen.getByRole("button", { name: "Изменить сочетание: Окно быстрого захвата" }));
    fireEvent.keyDown(document.activeElement!, { key: "Escape", code: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Изменить сочетание: Окно быстрого захвата" })).toHaveFocus();
    fireEvent.keyDown(document.activeElement!, { key: "Escape", code: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("английский: раздел сочетаний", () => {
    view({}, "en");
    expect(screen.getByRole("region", { name: "Keyboard shortcuts" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Quick capture window" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Save clipboard to Inbox" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Change shortcut: Quick capture window" })).toBeInTheDocument();
  });

  it("английский: ошибка копии", () => {
    view({ settings: settings({ error: "write_failed" }) }, "en");
    expect(screen.getByRole("alert")).toHaveTextContent("Could not write the copy.");
  });
});
