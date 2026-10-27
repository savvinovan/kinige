import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import type { StartupState } from "../../api/generated/StartupState";
import { act, fireEvent, render, screen, waitFor } from "../../test/render";
import { AppRoot } from "./AppRoot";

const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const BACKUP = { name: "kinige-2026-10-20.db", kind: "daily" as const, createdAt: Date.UTC(2026, 9, 20, 6, 5) };

const NORMAL: StartupState = {
  needsRecovery: false,
  backups: [],
  databaseTooNew: false,
  corruptionFound: false,
};

function setup(startup: StartupState) {
  const api = new MockApi({ now: NOW, startup });
  render(<AppRoot api={api} initialSettings={DEFAULT_SETTINGS} startup={startup} />);
  return api;
}

describe("AppRoot: запуск и восстановление (слайс 3)", () => {
  it("база в порядке: главное окно, окна восстановления нет", async () => {
    setup(NORMAL);
    expect(await screen.findByRole("textbox", { name: "Новая заметка" })).toBeInTheDocument();
    expect(screen.queryByRole("main", { name: "База повреждена" })).toBeNull();
  });

  it("база повреждена при запуске: окно с копиями вместо журнала; восстановление открывает журнал", async () => {
    const api = setup({ ...NORMAL, needsRecovery: true, backups: [BACKUP] });
    expect(await screen.findByRole("main", { name: "База повреждена" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Новая заметка" })).toBeNull();
    // Папка Markdown-копии будет отложена в сторону: человек видит это до нажатия.
    expect(screen.getByText(/Папка Markdown-копии будет переименована в «…-before-restore-…»/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^Восстановить из копии/ }));
    expect(await screen.findByRole("textbox", { name: "Новая заметка" })).toBeInTheDocument();
    expect(api.restoredFrom).toBe(BACKUP.name);
  });

  it("повреждение найдено во время восстановления: после него окно «перезапустить», а не журнал на повреждённой базе", async () => {
    const api = setup({ ...NORMAL, needsRecovery: true, backups: [BACKUP] });
    await screen.findByRole("main", { name: "База повреждена" });
    // Событие приходит в окно «startup» и игнорируется; к концу восстановления Rust уже знает о находке.
    const restoreBackup = api.restoreBackup.bind(api);
    api.restoreBackup = async (name) => {
      const outcome = await restoreBackup(name);
      act(() => api.emitDatabaseCorrupt());
      api.startupState = () => Promise.resolve({ ...NORMAL, corruptionFound: true });
      return outcome;
    };
    fireEvent.click(screen.getByRole("button", { name: /^Восстановить из копии/ }));
    expect(await screen.findByRole("button", { name: "Перезапустить" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Новая заметка" })).toBeNull();
  });

  it("ошибка восстановления: окно остаётся, текст ошибки виден", async () => {
    const api = setup({ ...NORMAL, needsRecovery: true, backups: [BACKUP] });
    await screen.findByRole("main", { name: "База повреждена" });
    api.failNext("restoreBackup", "backup_not_found");
    fireEvent.click(screen.getByRole("button", { name: /^Восстановить из копии/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Копия не найдена или не открывается.");
    expect(screen.getByRole("main", { name: "База повреждена" })).toBeInTheDocument();
  });

  it("повреждение найдено в работающей базе: окно со «Перезапустить», кнопка просит перезапуск", async () => {
    const api = setup(NORMAL);
    await screen.findByRole("textbox", { name: "Новая заметка" });
    await waitFor(() => {
      act(() => api.emitDatabaseCorrupt());
      expect(screen.getByRole("main", { name: "База повреждена" })).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "Перезапустить" }));
    await waitFor(() => expect(api.restartRequested).toBe(true));
  });

  it("повреждение найдено до подписки страницы на событие: отметка в состоянии запуска открывает окно без события", async () => {
    const api = new MockApi({ now: NOW });
    // Страница получила состояние до проверки, а к моменту подписки Rust уже поставил отметку.
    api.startupState = () => Promise.resolve({ ...NORMAL, corruptionFound: true });
    render(<AppRoot api={api} initialSettings={DEFAULT_SETTINGS} startup={NORMAL} />);
    expect(await screen.findByRole("main", { name: "База повреждена" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Перезапустить" })).toBeInTheDocument();
  });

  it("база создана более новой версией: сообщение без кнопок, журнал не открыт", async () => {
    setup({ ...NORMAL, databaseTooNew: true });
    expect(await screen.findByText("База создана более новой версией Kinige")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByRole("textbox", { name: "Новая заметка" })).toBeNull();
  });
});
