import { afterEach, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../api/KinigeApi";
import { mockMessage } from "../api/MockApi";
import { MOCK_SETTINGS_FILE } from "../api/MockApiSlice4b";
import type { AgentLogEntry } from "../api/generated/AgentLogEntry";
import { act, fireEvent, screen, waitFor, within } from "../test/render";
import { openWindow } from "../test/prefsWindow";

afterEach(() => vi.useRealTimers());

const tree = () => screen.getByRole("navigation", { name: /Разделы настроек|Settings sections/ });
const AT = Date.UTC(2026, 9, 28, 11, 2);
const ENABLE = "Разрешить агентам доступ к журналу";
const WARNING = "Включить доступ агентов?";

async function openAccess(options: Parameters<typeof openWindow>[0] = {}, language: "ru" | "en" = "ru") {
  const api = await openWindow(options, language);
  fireEvent.click(within(tree()).getByRole("button", { name: /Доступ ИИ-агентов|AI agent access/ }));
  return { api, page: await screen.findByRole("region", { name: /Доступ ИИ-агентов|AI agent access/ }) };
}

async function openSubscreen(name: RegExp, options: Parameters<typeof openWindow>[0] = {}) {
  const { api } = await openAccess(options);
  fireEvent.click(await screen.findByRole("button", { name }));
  return { api, page: await screen.findByRole("region", { name: /Видимые чаты|Журнал обращений/ }) };
}

describe("Доступ ИИ-агентов: страница и предупреждение", () => {
  it("доступ выключен: режим недоступен, журнал и чаты — на подэкранах, подсказка про переподключение", async () => {
    const { page } = await openAccess();
    expect(within(page).getByRole("checkbox", { name: ENABLE })).not.toBeChecked();
    for (const option of within(page).getAllByRole("radio")) expect(option).toBeDisabled();
    expect(within(page).getByText("После смены режима переподключите агента.")).toBeInTheDocument();
    // Длинные списки не на этой странице.
    expect(within(page).queryByRole("checkbox", { name: /Входящие/ })).toBeNull();
    expect(within(page).getByRole("button", { name: /Видимые чаты.*0 из 3.*Настроить/ })).toBeInTheDocument();
    expect(within(page).getByRole("button", { name: /Журнал обращений.*Настроить/ })).toBeInTheDocument();
  });

  it("включение: сначала предупреждение поверх окна, «Отмена» ничего не включает, «Включить» включает", async () => {
    const { api, page } = await openAccess();
    fireEvent.click(within(page).getByRole("checkbox", { name: ENABLE }));
    const warning = await screen.findByRole("dialog", { name: WARNING });
    expect(warning).toHaveTextContent("Текст, который прочитает агент, уйдёт провайдеру его модели. Сквозное шифрование Kinige на это не распространяется.");
    await waitFor(() => expect(within(warning).getByRole("button", { name: "Отмена" })).toHaveFocus());
    fireEvent.click(within(warning).getByRole("button", { name: "Отмена" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: WARNING })).toBeNull());
    expect(api.agentEnabled).toBe(false);

    fireEvent.click(within(page).getByRole("checkbox", { name: ENABLE }));
    fireEvent.click(within(await screen.findByRole("dialog", { name: WARNING })).getByRole("button", { name: "Включить" }));
    await waitFor(() => expect(api.agentEnabled).toBe(true));
    await waitFor(() => expect(within(page).getByRole("checkbox", { name: ENABLE })).toBeChecked());
    for (const option of within(page).getAllByRole("radio")) expect(option).toBeEnabled();
  });

  it("Esc закрывает предупреждение, окно настроек остаётся", async () => {
    const { api, page } = await openAccess();
    fireEvent.click(within(page).getByRole("checkbox", { name: ENABLE }));
    await screen.findByRole("dialog", { name: WARNING });
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: WARNING })).toBeNull());
    expect(tree()).toBeInTheDocument();
    expect(api.agentEnabled).toBe(false);
  });

  it("предупреждение — один Modal поверх окна: прокручиваемая страница под ним ничего не меняет", async () => {
    const { page } = await openAccess();
    const scroller = document.querySelector<HTMLElement>(".prefs__page");
    const before = scroller?.className;
    fireEvent.click(within(page).getByRole("checkbox", { name: ENABLE }));
    await screen.findByRole("dialog", { name: WARNING });
    expect(document.querySelectorAll(".modal")).toHaveLength(1);
    expect(document.querySelector(".prefs__page")).toBe(scroller);
    expect(scroller?.className).toBe(before);
    expect(scroller?.hasAttribute("style")).toBe(false);
    expect(document.body.style.overflow).toBe("");
    expect(document.documentElement.style.overflow).toBe("");
  });

  it("выключение не требует предупреждения; режим меняется одним переключателем", async () => {
    const { api, page } = await openAccess();
    api.agentEnabled = true;
    act(() => api.emitSettingsChanged());
    await waitFor(() => expect(within(page).getByRole("checkbox", { name: ENABLE })).toBeChecked());
    fireEvent.click(within(page).getByRole("radio", { name: "Чтение и запись" }));
    await waitFor(() => expect(api.agentMode).toBe("read_write"));
    fireEvent.click(within(page).getByRole("checkbox", { name: ENABLE }));
    await waitFor(() => expect(api.agentEnabled).toBe(false));
    expect(screen.queryByRole("dialog", { name: WARNING })).toBeNull();
  });

  it("ошибка действия видна на странице", async () => {
    const { api, page } = await openAccess();
    api.agentEnabled = true;
    act(() => api.emitSettingsChanged());
    await waitFor(() => expect(within(page).getByRole("checkbox", { name: ENABLE })).toBeChecked());
    api.failNext("setAgentMode", "store_write_failed");
    fireEvent.click(within(page).getByRole("radio", { name: "Чтение и запись" }));
    expect(await within(page).findByRole("alert")).toHaveTextContent("Не удалось сохранить запись");
  });

  it("«Как подключить»: фрагмент и команда с кнопками «Копировать»", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const { api, page } = await openAccess();
    fireEvent.click(within(page).getByRole("button", { name: "Копировать конфиг для Claude Desktop" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(api.agentSocket.desktopConfig));
    fireEvent.click(within(page).getByRole("button", { name: "Копировать команду для Claude Code" }));
    await waitFor(() => expect(writeText).toHaveBeenLastCalledWith(api.agentSocket.codeCommand));
  });

  it.each([
    ["dir_unsafe", /Папка сокета для агентов чужая/],
    ["bridge_missing", /лежит не рядом с приложением/],
  ])("ошибка сокета «%s»: одна строка ошибки вместо блока «Как подключить»", async (code, text) => {
    const { api, page } = await openAccess();
    api.agentSocket = { ...api.agentSocket, listening: false, error: code, desktopConfig: null, codeCommand: null };
    act(() => api.emitSettingsChanged());
    expect(await within(page).findByRole("alert")).toHaveTextContent(text);
    expect(within(page).queryByRole("button", { name: /Копировать/ })).toBeNull();
  });

  it("«Изменено через settings.json · время» рядом с доступом; в журнале строка «доступ изменён»", async () => {
    const { api, page } = await openAccess();
    api.agentEnabled = true;
    api.settingsFile = { ...MOCK_SETTINGS_FILE, changed: { appearance: null, shortcuts: null, mirror: null, access: AT } };
    api.addAgentLog({ tool: "settings_file", client: "settings.json", at: AT, count: 0 });
    act(() => api.emitSettingsChanged());
    expect(await within(page).findByText(/Изменено через settings\.json · .*\d{2}:\d{2}/)).toBeInTheDocument();
    expect(within(page).getByRole("checkbox", { name: ENABLE })).toBeChecked();
    fireEvent.click(within(page).getByRole("button", { name: /Журнал обращений.*Настроить/ }));
    const log = await screen.findByRole("region", { name: "Журнал обращений" });
    expect(within(log).getByText(/settings\.json · доступ изменён/)).toBeInTheDocument();
  });

  it("на английском: те же элементы", async () => {
    const { page } = await openAccess({}, "en");
    expect(within(page).getByRole("checkbox", { name: "Allow agents to access the journal" })).toBeInTheDocument();
    expect(within(page).getByRole("radio", { name: "Read and write" })).toBeInTheDocument();
    expect(within(page).getByRole("button", { name: /Visible chats.*Configure/ })).toBeInTheDocument();
  });
});

describe("Подэкран «Видимые чаты»", () => {
  it("список чатов целиком; чаты отмечаются, чат ai_excluded неактивен с пометкой; доступ выключен — всё недоступно", async () => {
    const { api, page } = await openSubscreen(/Видимые чаты.*Настроить/);
    for (const box of within(page).getAllByRole("checkbox")) expect(box).toBeDisabled();
    api.agentEnabled = true;
    act(() => api.emitSettingsChanged());
    const inbox = await waitFor(() => {
      const box = within(page).getByRole("checkbox", { name: /Входящие/ });
      expect(box).toBeEnabled();
      return box;
    });
    const work = within(page).getByRole("checkbox", { name: /Работа/ });
    expect(inbox).not.toBeChecked();
    fireEvent.click(inbox);
    await waitFor(() => expect(api.agentChats).toEqual([INBOX_ID]));
    fireEvent.click(work);
    await waitFor(() => expect(api.agentChats).toEqual([INBOX_ID, "work"]));
    await waitFor(() => expect(within(page).getByRole("checkbox", { name: /Работа/ })).toBeChecked());
    fireEvent.click(within(page).getByRole("checkbox", { name: /Входящие/ }));
    await waitFor(() => expect(api.agentChats).toEqual(["work"]));
    expect(within(page).getByRole("checkbox", { name: /Дневник/ })).toBeDisabled();
    expect(within(page).getByText(/исключён из AI/)).toBeInTheDocument();
  });

  it("отметка чата файлом видна без перезапуска", async () => {
    const { api, page } = await openSubscreen(/Видимые чаты.*Настроить/);
    api.agentEnabled = true;
    api.agentChats = ["work"];
    act(() => api.emitSettingsChanged());
    await waitFor(() => expect(within(page).getByRole("checkbox", { name: /Работа/ })).toBeChecked());
  });
});

describe("Подэкран «Журнал обращений»", () => {
  const entry = (patch: Partial<AgentLogEntry> & Pick<AgentLogEntry, "messageId">): AgentLogEntry => ({
    message: null,
    chatTitle: "Работа",
    chatIcon: "💼",
    chatIsInbox: false,
    ...patch,
  });
  const rowsOf = (log: HTMLElement) => within(log).getAllByRole("listitem").filter((li) => li.className.includes("agent-log__row"));

  it("пуст: «Обращений не было»", async () => {
    const { page } = await openSubscreen(/Журнал обращений.*Настроить/);
    expect(within(page).getByText("Обращений не было")).toBeInTheDocument();
  });

  it("строки: время · клиент · действие · N записей; отказ говорит причину; имя клиента без значка «проверено»", async () => {
    const opened = await openAccess();
    opened.api.addAgentLog({ tool: "search", query: "Raft", at: AT, count: 3 }, [entry({ messageId: "a" }), entry({ messageId: "b" }), entry({ messageId: "c" })]);
    opened.api.addAgentLog({ tool: "add_entry", at: AT + 3 * 60_000, outcome: "read_only", count: 0 });
    opened.api.addAgentLog({ tool: "connect", client: "Evil <b>Client</b>", at: AT + 4 * 60_000, count: 0 });
    act(() => opened.api.emitSettingsChanged());
    fireEvent.click(await screen.findByRole("button", { name: /Журнал обращений.*Настроить/ }));
    const log = await screen.findByRole("region", { name: "Журнал обращений" });
    await waitFor(() => expect(rowsOf(log)).toHaveLength(3));
    const when = String.raw`^(\d+ [^,]+, )?\d{2}:\d{2}`;
    expect(rowsOf(log)[0]).toHaveTextContent(new RegExp(`${when} · Evil <b>Client</b> · подключение$`));
    expect(rowsOf(log)[1]).toHaveTextContent(new RegExp(`${when} · Claude Desktop · добавить запись · отказано: только чтение$`));
    expect(rowsOf(log)[2]).toHaveTextContent(new RegExp(`${when} · Claude Desktop · поиск «Raft» · 3 записи$`));
    expect(within(log).queryByText(/проверено|verified/i)).toBeNull();
  });

  it("подпись «доступ изменён» — по инструменту, а не по имени клиента: клиент «settings.json» с поиском — это поиск", async () => {
    const opened = await openAccess();
    opened.api.addAgentLog({ tool: "search", query: "Raft", client: "settings.json", at: AT, count: 0 });
    opened.api.addAgentLog({ tool: "settings_file", client: "settings.json", at: AT + 60_000, count: 0 });
    act(() => opened.api.emitSettingsChanged());
    fireEvent.click(await screen.findByRole("button", { name: /Журнал обращений.*Настроить/ }));
    const log = await screen.findByRole("region", { name: "Журнал обращений" });
    await waitFor(() => expect(rowsOf(log)).toHaveLength(2));
    expect(rowsOf(log)[0]).toHaveTextContent(/settings\.json · доступ изменён$/);
    expect(rowsOf(log)[1]).toHaveTextContent(/settings\.json · поиск «Raft»$/);
    expect(rowsOf(log)[1]).not.toHaveTextContent("доступ изменён");
  });

  it("раскрытие показывает записи; удалённая навсегда запись — тихая строка «Запись удалена»", async () => {
    const opened = await openAccess();
    const kept = mockMessage({ id: "m1", chatId: "work", bodyMd: "Итоги встречи по Raft", journalDate: "2026-03-14" });
    opened.api.addAgentLog({ tool: "get_entries", at: AT, count: 2 }, [entry({ messageId: "m1", message: kept }), entry({ messageId: "gone" })]);
    act(() => opened.api.emitSettingsChanged());
    fireEvent.click(await screen.findByRole("button", { name: /Журнал обращений.*Настроить/ }));
    const log = await screen.findByRole("region", { name: "Журнал обращений" });
    const row = await within(log).findByRole("button", { name: /чтение записей/ });
    expect(row).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(row);
    expect(await within(log).findByText("Итоги встречи по Raft")).toBeInTheDocument();
    expect(within(log).getByText(/Работа · 14 мар/)).toBeInTheDocument();
    expect(within(log).getByText("Запись удалена")).toBeInTheDocument();
    fireEvent.click(row);
    await waitFor(() => expect(within(log).queryByText("Итоги встречи по Raft")).toBeNull());
  });

  it("после «Удалить навсегда» число в строке прежнее, раскрытие говорит, сколько записей удалено", async () => {
    const opened = await openAccess();
    const kept = mockMessage({ id: "m1", chatId: "work", bodyMd: "Осталась", journalDate: "2026-03-14" });
    opened.api.addAgentLog({ tool: "search", query: "Raft", at: AT, count: 3 }, [entry({ messageId: "m1", message: kept })]);
    act(() => opened.api.emitSettingsChanged());
    fireEvent.click(await screen.findByRole("button", { name: /Журнал обращений.*Настроить/ }));
    const log = await screen.findByRole("region", { name: "Журнал обращений" });
    const row = await within(log).findByRole("button", { name: /поиск/ });
    expect(row).toHaveTextContent(/3 записи$/);
    fireEvent.click(row);
    expect(await within(log).findByText("Осталась")).toBeInTheDocument();
    expect(within(log).getByText("ещё 2 записи удалены навсегда")).toBeInTheDocument();
  });

  it("щелчок по записи просит открыть её в главном окне; окно настроек остаётся на месте", async () => {
    const message = mockMessage({ id: "m1", chatId: "work", bodyMd: "Итоги встречи по Raft", journalDate: "2026-10-27", origin: "agent" });
    const opened = await openAccess({ messages: [message] });
    opened.api.addAgentLog({ tool: "add_entry", at: AT, count: 1 }, [entry({ messageId: "m1", message })]);
    act(() => opened.api.emitSettingsChanged());
    fireEvent.click(await screen.findByRole("button", { name: /Журнал обращений.*Настроить/ }));
    const log = await screen.findByRole("region", { name: "Журнал обращений" });
    fireEvent.click(await within(log).findByRole("button", { name: /добавить запись/ }));
    fireEvent.click(await within(log).findByRole("button", { name: /Итоги встречи по Raft/ }));
    await waitFor(() => expect(opened.api.openedEntries).toEqual(["m1"]));
    expect(screen.getByRole("region", { name: "Журнал обращений" })).toBeInTheDocument();
  });

  it("запись удалена навсегда после чтения журнала: на месте записи — «Запись удалена», Rust отказал без текста", async () => {
    const orphan = mockMessage({ id: "ghost", chatId: "work", bodyMd: "Призрак", journalDate: "2026-10-27" });
    const opened = await openAccess();
    opened.api.addAgentLog({ tool: "get_entry", at: AT, count: 1 }, [entry({ messageId: "ghost", message: orphan })]);
    act(() => opened.api.emitSettingsChanged());
    fireEvent.click(await screen.findByRole("button", { name: /Журнал обращений.*Настроить/ }));
    const log = await screen.findByRole("region", { name: "Журнал обращений" });
    fireEvent.click(await within(log).findByRole("button", { name: /чтение записи/ }));
    fireEvent.click(await within(log).findByRole("button", { name: /Призрак/ }));
    await waitFor(() => expect(within(log).getByText("Запись удалена")).toBeInTheDocument());
    expect(within(log).queryByRole("button", { name: /Призрак/ })).toBeNull();
    expect(opened.api.openedEntries).toEqual([]);
  });

  it("«Показать ещё» внизу страницы дочитывает старые строки страницами", async () => {
    const opened = await openAccess();
    for (let i = 0; i < 35; i += 1) opened.api.addAgentLog({ tool: "list_chats", at: AT + i * 1000, count: 0 });
    act(() => opened.api.emitSettingsChanged());
    fireEvent.click(await screen.findByRole("button", { name: /Журнал обращений.*Настроить/ }));
    const log = await screen.findByRole("region", { name: "Журнал обращений" });
    await waitFor(() => expect(rowsOf(log)).toHaveLength(30));
    fireEvent.click(within(log).getByRole("button", { name: "Показать ещё" }));
    await waitFor(() => expect(rowsOf(log)).toHaveLength(35));
    expect(within(log).queryByRole("button", { name: "Показать ещё" })).toBeNull();
  });

  it("журнал обновляется сам, пока окно открыто", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const opened = await openAccess();
    fireEvent.click(await screen.findByRole("button", { name: /Журнал обращений.*Настроить/ }));
    const log = await screen.findByRole("region", { name: "Журнал обращений" });
    expect(within(log).getByText("Обращений не было")).toBeInTheDocument();
    opened.api.addAgentLog({ tool: "list_chats", at: AT, count: 0 });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2100);
    });
    await waitFor(() => expect(within(log).getByText(/список чатов/)).toBeInTheDocument());
  });
});
