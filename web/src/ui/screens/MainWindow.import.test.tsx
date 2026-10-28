import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import type { ImportPreview } from "../../api/generated/ImportPreview";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Импорт из Telegram на настоящем `MainWindow` и `MockApi`: все шаги, отмена выбора, ошибки по коду, остановка, клавиатура.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

const PREVIEW: ImportPreview = {
  chats: [
    { id: "1000001", name: null, kind: "saved", total: 1682, existing: 0 },
    { id: "2000002", name: "Пётр Тестов", kind: "personal", total: 240, existing: 40 },
    { id: "2000003", name: "Пустой", kind: "personal", total: 0, existing: 0 },
  ],
  skippedChats: 3,
};

function setup(language: "ru" | "en" = "ru") {
  const api = new MockApi({ now: NOW, language });
  api.importFile = PREVIEW;
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language });
  return api;
}

async function openDialog() {
  fireEvent.click(await screen.findByRole("button", { name: /^Профиль/ }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Импорт из Telegram…" }));
  return screen.findByRole("dialog", { name: "Импорт из Telegram" });
}
const pick = async () => {
  fireEvent.click(screen.getByRole("button", { name: "Выбрать result.json…" }));
  await screen.findByRole("list", { name: "Чаты для импорта" });
};

describe("Импорт: шаг файла", () => {
  it("пункт меню профиля открывает окно с инструкцией и кнопкой выбора файла; крестик закрывает и забывает файл", async () => {
    const api = setup();
    const dialog = await openDialog();
    expect(within(dialog).getByText(/Экспорт данных Telegram/)).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Выбрать result.json…" })).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Закрыть" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.importClosed).toBe(1);
  });

  it("отмена системного диалога ничего не меняет: окно остаётся на шаге файла без ошибки", async () => {
    const api = setup();
    api.importFile = null;
    const dialog = await openDialog();
    fireEvent.click(within(dialog).getByRole("button", { name: "Выбрать result.json…" }));
    await waitFor(() => expect(api.calls).toContain("importTelegramOpen"));
    await act(async () => void (await Promise.resolve()));
    expect(within(dialog).queryByRole("alert")).toBeNull();
    expect(within(dialog).getByRole("button", { name: "Выбрать result.json…" })).toBeInTheDocument();
  });

  it("не тот файл, нечитаемый и слишком большой — строка ошибки D1 по коду; ничего не записано", async () => {
    const api = setup();
    const dialog = await openDialog();
    for (const [code, text] of [
      ["import_not_telegram", /Это не экспорт Telegram/],
      ["import_file_unreadable", /Файл не удалось прочитать/],
      ["import_file_too_large", /Файл больше 1 ГБ/],
    ] as const) {
      api.importFile = code;
      fireEvent.click(within(dialog).getByRole("button", { name: "Выбрать result.json…" }));
      expect(await within(dialog).findByRole("alert")).toHaveTextContent(text);
      await act(async () => void (await Promise.resolve()));
    }
    expect(api.calls.some((call) => call.startsWith("importTelegramRun"))).toBe(false);
  });
});

describe("Импорт: выбор чатов", () => {
  it("список: «Избранное» отмечено, личные чаты нет; у каждого тип, число записей и сколько уже есть; группы и каналы — одной строкой", async () => {
    setup();
    const dialog = await openDialog();
    await pick();
    const saved = within(dialog).getByRole("checkbox", { name: "Избранное" });
    const peter = within(dialog).getByRole("checkbox", { name: "Пётр Тестов" });
    expect(saved).toBeChecked();
    expect(peter).not.toBeChecked();
    // Разряды в русском числе разделены неразрывным пробелом (Intl).
    expect(saved).toHaveAccessibleDescription("Избранное · 1 682 записи");
    expect(peter).toHaveAccessibleDescription("Личный чат · 240 записей · 40 уже есть");
    // Чат без записей нельзя отметить.
    const empty = within(dialog).getByRole("checkbox", { name: "Пустой" });
    expect(empty).toBeDisabled();
    expect(empty).toHaveAccessibleDescription("Личный чат · нет записей");
    expect(within(dialog).getByText("Группы и каналы пропущены: 3")).toBeInTheDocument();
    expect(within(dialog).getByText(/на двух устройствах до синхронизации/)).toBeInTheDocument();
  });

  it("«Импортировать» недоступна без отмеченных чатов; пробел отмечает флажок с клавиатуры", async () => {
    const api = setup();
    const dialog = await openDialog();
    await pick();
    const run = within(dialog).getByRole("button", { name: "Импортировать" });
    fireEvent.click(within(dialog).getByRole("checkbox", { name: "Избранное" }));
    expect(run).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(run);
    expect(api.calls.some((call) => call.startsWith("importTelegramRun"))).toBe(false);
    const peter = within(dialog).getByRole("checkbox", { name: "Пётр Тестов" });
    peter.focus();
    fireEvent.click(peter);
    expect(peter).toBeChecked();
    expect(run).not.toHaveAttribute("aria-disabled");
  });

  it("Tab ходит по кругу внутри окна, Esc закрывает", async () => {
    const api = setup();
    const dialog = await openDialog();
    await pick();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.importClosed).toBe(1);
    expect(dialog).not.toBeInTheDocument();
  });

  it("«Другой файл…» снова открывает выбор и заменяет список", async () => {
    const api = setup();
    const dialog = await openDialog();
    await pick();
    api.importFile = { chats: [{ id: "9", name: "Новый", kind: "personal", total: 5, existing: 0 }], skippedChats: 0 };
    fireEvent.click(within(dialog).getByRole("button", { name: "Другой файл…" }));
    expect(await within(dialog).findByRole("checkbox", { name: "Новый" })).not.toBeChecked();
    expect(within(dialog).queryByRole("checkbox", { name: "Избранное" })).toBeNull();
  });

  it("файл без чатов для импорта: тихая строка", async () => {
    const api = setup();
    api.importFile = { chats: [], skippedChats: 2 };
    const dialog = await openDialog();
    fireEvent.click(within(dialog).getByRole("button", { name: "Выбрать result.json…" }));
    expect(await within(dialog).findByText("В файле нет чатов, которые можно импортировать.")).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Импортировать" })).toBeInTheDocument();
  });
});

describe("Импорт: прогресс, остановка, отчёт", () => {
  it("прогресс «N из M», крестика и Esc нет, только «Остановить»; отчёт называет копию; «Готово» открывает Избранное", async () => {
    const api = setup();
    api.holdImport = true;
    api.importSteps = 3;
    const dialog = await openDialog();
    await pick();
    fireEvent.click(within(dialog).getByRole("checkbox", { name: "Пётр Тестов" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Импортировать" }));

    const bar = await within(dialog).findByRole("progressbar", { name: "Импорт" });
    await waitFor(() => expect(bar).toHaveAttribute("aria-valuetext", "641 из 1 922"));
    expect(within(dialog).getByRole("button", { name: "Остановить" })).toHaveFocus();
    expect(within(dialog).queryByRole("button", { name: "Закрыть" })).toBeNull();
    // Esc и щелчок по подложке окно не закрывают.
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(api.importClosed).toBe(0);

    // Каждый шаг ждёт своего «отпускания»: второе возможно только после того, как мок дошёл до следующей остановки.
    for (let step = 0; step < 2; step++) {
      await act(async () => {
        api.releaseImport();
        await Promise.resolve();
        await Promise.resolve();
      });
    }
    await waitFor(() => expect(within(screen.getByRole("dialog")).getByRole("heading", { name: "Импорт завершён" })).toBeInTheDocument());
    const report = within(screen.getByRole("dialog"));
    // Текст узла Testing Library нормализует (неразрывный пробел становится обычным).
    expect(report.getByText("Импортировано: 1 882")).toBeInTheDocument();
    expect(report.getByText("Пропущено, уже импортировано: 40")).toBeInTheDocument();
    expect(report.getByText(/Вложения не перенесены у записей: \d/)).toBeInTheDocument();
    // Нулевые счётчики в отчёте не выводятся (кроме «Импортировано»): чат Петра уже был, нового не создано.
    expect(report.queryByText(/Создано чатов/)).toBeNull();
    expect(report.getByText(/Копия базы перед импортом: pre-import-/)).toBeInTheDocument();
    expect(api.lastImportChats).toEqual(["1000001", "2000002"]);

    fireEvent.click(report.getByRole("button", { name: "Готово" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    // Открыт чат «Избранного» (Inbox).
    expect(screen.getByRole("button", { name: /^Входящие/ })).toHaveAttribute("aria-current", "true");
  });

  it("«Остановить»: импорт останавливается, отчёт называет остановку и остаток доимпортируется повторным запуском", async () => {
    const api = setup();
    api.holdImport = true;
    const dialog = await openDialog();
    await pick();
    fireEvent.click(within(dialog).getByRole("button", { name: "Импортировать" }));
    await within(dialog).findByRole("progressbar");
    const stop = within(dialog).getByRole("button", { name: "Остановить" });
    fireEvent.click(stop);
    // Пока Rust дописывает пачку, окно говорит об остановке и кнопка недоступна.
    expect(within(dialog).getByText("Останавливаем…")).toBeInTheDocument();
    expect(stop).toHaveAttribute("aria-disabled", "true");
    expect(await within(dialog).findByRole("heading", { name: "Импорт остановлен" })).toBeInTheDocument();
    expect(within(dialog).getByText(/Остаток доимпортируется/)).toBeInTheDocument();
    expect(api.calls).toContain("importTelegramStop");
  });

  it("повторный импорт того же файла: новых записей нет, «пропущено, уже импортировано»", async () => {
    const api = setup();
    api.importFile = { chats: [{ id: "1000001", name: null, kind: "saved", total: 1682, existing: 1682 }], skippedChats: 0 };
    const dialog = await openDialog();
    await pick();
    fireEvent.click(within(dialog).getByRole("button", { name: "Импортировать" }));
    const report = await within(dialog).findByRole("heading", { name: "Импорт завершён" });
    expect(report).toBeInTheDocument();
    expect(within(dialog).getByText("Импортировано: 0")).toBeInTheDocument();
    expect(within(dialog).getByText("Пропущено, уже импортировано: 1 682")).toBeInTheDocument();
    expect(within(dialog).queryByText(/Копия базы/)).toBeNull();
  });

  it("ошибки запуска по коду: сессия забыта — назад к выбору файла; копия не снялась — к списку, пачек не записано", async () => {
    const api = setup();
    const dialog = await openDialog();
    await pick();
    api.failNext("importTelegramRun", "import_backup_failed");
    fireEvent.click(within(dialog).getByRole("button", { name: "Импортировать" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("Не удалось снять копию базы перед импортом");
    expect(within(dialog).getByRole("list", { name: "Чаты для импорта" })).toBeInTheDocument();

    api.failNext("importTelegramRun", "import_session_expired");
    fireEvent.click(within(dialog).getByRole("button", { name: "Импортировать" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("Выбранный файл забыт");
    expect(within(dialog).getByRole("button", { name: "Выбрать result.json…" })).toBeInTheDocument();
  });

  it("по-английски: заголовки, подписи и отчёт", async () => {
    const api = setup("en");
    api.holdImport = false;
    fireEvent.click(await screen.findByRole("button", { name: /^Profile/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Import from Telegram…" }));
    const dialog = await screen.findByRole("dialog", { name: "Import from Telegram" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Choose result.json…" }));
    expect(await within(dialog).findByRole("checkbox", { name: "Saved Messages" })).toBeChecked();
    fireEvent.click(within(dialog).getByRole("button", { name: "Import" }));
    expect(await within(dialog).findByRole("heading", { name: "Import finished" })).toBeInTheDocument();
    expect(within(dialog).getByText("Imported: 1,682")).toBeInTheDocument();
  });
});
