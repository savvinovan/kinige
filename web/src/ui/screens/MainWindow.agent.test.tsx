import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, MockApi, type MockApiOptions, mockMessage } from "../../api/MockApi";
import { INBOX_ID } from "../../api/KinigeApi";
import type { AgentLogEntry } from "../../api/generated/AgentLogEntry";
import type { Settings } from "../../api/generated/Settings";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { DEFAULT_SHORTCUTS } from "../settings/demoShortcuts";
import { MainWindow } from "./MainWindow";

// Слайс 4a на MockApi: строка в настройках, окно «Доступ ИИ-агентов», предупреждение, режим, чаты, «Копировать», журнал,
// открытие записи из журнала и пометка «добавлено агентом» в ленте.

const SETTINGS: Settings = {
  ...DEFAULT_SETTINGS,
  mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
  shortcuts: DEFAULT_SHORTCUTS,
};
const NOW = () => new Date(Date.UTC(2026, 9, 28, 12, 0));
const AT = Date.UTC(2026, 9, 28, 11, 2);

function setup(options: MockApiOptions = {}, language: "ru" | "en" = "ru") {
  const api = new MockApi({
    now: NOW,
    chats: [{ id: "work", title: "Работа", icon: "💼" }, { id: "diary", title: "Дневник", icon: "📓" }],
    ...options,
  });
  api.agentExcluded.add("diary");
  render(<MainWindow api={api} initialSettings={SETTINGS} />, { language });
  return api;
}

async function openAgentDialog() {
  fireEvent.click(await screen.findByRole("button", { name: /Профиль|Profile/ }));
  fireEvent.click(await screen.findByRole("menuitem", { name: /Настройки|Settings/ }));
  const settings = await screen.findByRole("dialog", { name: /Настройки|Settings/ });
  await within(settings).findByText(/^(Выключен|Включён|Off|On)$/);
  fireEvent.click(within(settings).getByRole("button", { name: /Настроить…|Configure…/ }));
  return screen.findByRole("dialog", { name: /Доступ ИИ-агентов|AI agent access/ });
}

afterEach(() => document.documentElement.removeAttribute("data-text-scale"));

describe("Доступ ИИ-агентов: настройки и предупреждение", () => {
  it("строка в настройках говорит «выключен» и ведёт в окно; в окне доступ выключен, режим и чаты недоступны, журнал виден", async () => {
    setup();
    const dialog = await openAgentDialog();
    const enable = within(dialog).getByRole("checkbox", { name: "Разрешить агентам доступ к журналу" });
    expect(enable).not.toBeChecked();
    for (const option of within(dialog).getAllByRole("radio")) expect(option).toBeDisabled();
    expect(within(dialog).getByRole("checkbox", { name: /Входящие/ })).toBeDisabled();
    expect(within(dialog).getByText("Обращений не было")).toBeInTheDocument();
    expect(within(dialog).getByText("После смены режима переподключите агента.")).toBeInTheDocument();
  });

  it("включение: сначала предупреждение, «Отмена» ничего не включает, «Включить» включает", async () => {
    const api = setup();
    const dialog = await openAgentDialog();
    fireEvent.click(within(dialog).getByRole("checkbox", { name: "Разрешить агентам доступ к журналу" }));
    const warning = await screen.findByRole("dialog", { name: "Включить доступ агентов?" });
    expect(warning).toHaveTextContent("Текст, который прочитает агент, уйдёт провайдеру его модели. Сквозное шифрование Kinige на это не распространяется.");
    // Фокус по умолчанию на «Отмена».
    await waitFor(() => expect(within(warning).getByRole("button", { name: "Отмена" })).toHaveFocus());
    fireEvent.click(within(warning).getByRole("button", { name: "Отмена" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Включить доступ агентов?" })).toBeNull());
    expect(api.agentEnabled).toBe(false);

    fireEvent.click(within(dialog).getByRole("checkbox", { name: "Разрешить агентам доступ к журналу" }));
    fireEvent.click(within(await screen.findByRole("dialog", { name: "Включить доступ агентов?" })).getByRole("button", { name: "Включить" }));
    await waitFor(() => expect(api.agentEnabled).toBe(true));
    await waitFor(() => expect(within(dialog).getByRole("checkbox", { name: "Разрешить агентам доступ к журналу" })).toBeChecked());
    for (const option of within(dialog).getAllByRole("radio")) expect(option).toBeEnabled();
  });

  it("Esc закрывает сначала предупреждение, потом окно", async () => {
    const api = setup();
    const dialog = await openAgentDialog();
    fireEvent.click(within(dialog).getByRole("checkbox", { name: "Разрешить агентам доступ к журналу" }));
    await screen.findByRole("dialog", { name: "Включить доступ агентов?" });
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Включить доступ агентов?" })).toBeNull());
    expect(screen.getByRole("dialog", { name: "Доступ ИИ-агентов" })).toBeInTheDocument();
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Доступ ИИ-агентов" })).toBeNull());
    expect(api.agentEnabled).toBe(false);
  });

  it("выключение не требует предупреждения", async () => {
    const api = setup();
    api.agentEnabled = true;
    const dialog = await openAgentDialog();
    fireEvent.click(within(dialog).getByRole("checkbox", { name: "Разрешить агентам доступ к журналу" }));
    await waitFor(() => expect(api.agentEnabled).toBe(false));
    expect(screen.queryByRole("dialog", { name: "Включить доступ агентов?" })).toBeNull();
  });

  it("режим меняется одним переключателем; чаты отмечаются, чат ai_excluded неактивен с пометкой", async () => {
    const api = setup();
    api.agentEnabled = true;
    const dialog = await openAgentDialog();
    fireEvent.click(within(dialog).getByRole("radio", { name: "Чтение и запись" }));
    await waitFor(() => expect(api.agentMode).toBe("read_write"));
    // Сразу не отмечено ничего, даже Входящие.
    const inbox = within(dialog).getByRole("checkbox", { name: /Входящие/ });
    const work = within(dialog).getByRole("checkbox", { name: /Работа/ });
    expect(inbox).not.toBeChecked();
    expect(work).not.toBeChecked();
    fireEvent.click(inbox);
    await waitFor(() => expect(api.agentChats).toEqual([INBOX_ID]));
    fireEvent.click(work);
    await waitFor(() => expect(api.agentChats).toEqual([INBOX_ID, "work"]));
    await waitFor(() => expect(within(dialog).getByRole("checkbox", { name: /Работа/ })).toBeChecked());
    fireEvent.click(within(dialog).getByRole("checkbox", { name: /Входящие/ }));
    await waitFor(() => expect(api.agentChats).toEqual(["work"]));
    const diary = within(dialog).getByRole("checkbox", { name: /Дневник/ });
    expect(diary).toBeDisabled();
    expect(within(dialog).getByText(/исключён из AI/)).toBeInTheDocument();
  });

  it("«Как подключить»: фрагмент и команда, у каждого «Копировать»; кнопка говорит «Скопировано»", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const api = setup();
    const dialog = await openAgentDialog();
    const desktop = within(dialog).getByRole("button", { name: "Копировать конфиг для Claude Desktop" });
    fireEvent.click(desktop);
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(api.agentSocket.desktopConfig));
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Скопировано" })).toBeInTheDocument());
    fireEvent.click(within(dialog).getByRole("button", { name: "Копировать команду для Claude Code" }));
    await waitFor(() => expect(writeText).toHaveBeenLastCalledWith(api.agentSocket.codeCommand));
    expect(within(dialog).getByText(/claude mcp add --scope user kinige/)).toBeInTheDocument();
  });

  it.each([
    ["dir_unsafe", /Папка сокета для агентов чужая/],
    ["second_instance", /уже работает другой Kinige/],
    ["bind_failed", /не смог открыть сокет/],
    ["bridge_missing", /лежит не рядом с приложением/],
  ])("ошибка сокета «%s»: одна строка ошибки на месте блока «Как подключить»", async (code, text) => {
    const api = setup();
    api.agentSocket = { ...api.agentSocket, listening: false, error: code, desktopConfig: null, codeCommand: null };
    const dialog = await openAgentDialog();
    expect(within(dialog).getByRole("alert")).toHaveTextContent(text);
    expect(within(dialog).queryByRole("button", { name: /Копировать/ })).toBeNull();
  });

  it("на английском: те же элементы", async () => {
    setup({}, "en");
    const dialog = await openAgentDialog();
    expect(within(dialog).getByRole("checkbox", { name: "Allow agents to access the journal" })).toBeInTheDocument();
    expect(within(dialog).getByText("No requests yet")).toBeInTheDocument();
    expect(within(dialog).getByRole("radio", { name: "Read and write" })).toBeInTheDocument();
  });
});

describe("Журнал обращений", () => {
  const entry = (patch: Partial<AgentLogEntry> & Pick<AgentLogEntry, "messageId">): AgentLogEntry => ({
    message: null,
    chatTitle: "Работа",
    chatIcon: "💼",
    chatIsInbox: false,
    ...patch,
  });

  it("строки: время · клиент · действие · N записей; отказ говорит причину; имя клиента без значка «проверено»", async () => {
    const api = setup();
    api.addAgentLog({ tool: "search", query: "Raft", at: AT, count: 3 }, [entry({ messageId: "a" }), entry({ messageId: "b" }), entry({ messageId: "c" })]);
    api.addAgentLog({ tool: "add_entry", at: AT + 3 * 60_000, outcome: "read_only", count: 0 });
    api.addAgentLog({ tool: "connect", client: "Evil <b>Client</b>", at: AT + 4 * 60_000, count: 0 });
    const dialog = await openAgentDialog();
    const log = within(dialog).getByRole("region", { name: "Журнал обращений" });
    const rows = within(log).getAllByRole("listitem").filter((li) => li.className.includes("agent-log__row"));
    expect(rows).toHaveLength(3);
    // Часы окна настоящие: строка вчерашнего дня начинается с даты («28 окт., 14:06»), сегодняшнего — со времени.
    const when = String.raw`^(\d+ [^,]+, )?\d{2}:\d{2}`;
    expect(rows[0]).toHaveTextContent(new RegExp(`${when} · Evil <b>Client</b> · подключение$`));
    expect(rows[1]).toHaveTextContent(new RegExp(`${when} · Claude Desktop · добавить запись · отказано: только чтение$`));
    expect(rows[2]).toHaveTextContent(new RegExp(`${when} · Claude Desktop · поиск «Raft» · 3 записи$`));
    expect(within(log).queryByText(/проверено|verified/i)).toBeNull();
    // Содержимого записей в журнале нет, пока строку не раскрыли.
    expect(within(log).queryByText("Итоги встречи")).toBeNull();
  });

  it("раскрытие показывает записи (чат, дата, начало текста); тихая строка «Запись удалена» для удалённых навсегда", async () => {
    const api = setup();
    const kept = mockMessage({ id: "m1", chatId: "work", bodyMd: "Итоги встречи по Raft", journalDate: "2026-03-14" });
    api.addAgentLog({ tool: "get_entries", at: AT, count: 2 }, [entry({ messageId: "m1", message: kept }), entry({ messageId: "gone" })]);
    const dialog = await openAgentDialog();
    const row = within(dialog).getByRole("button", { name: /получение|чтение записей/ });
    expect(row).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(row);
    await waitFor(() => expect(row).toHaveAttribute("aria-expanded", "true"));
    expect(await within(dialog).findByText("Итоги встречи по Raft")).toBeInTheDocument();
    expect(within(dialog).getByText(/Работа · 14 мар/)).toBeInTheDocument();
    expect(within(dialog).getByText("Запись удалена")).toBeInTheDocument();
    fireEvent.click(row);
    await waitFor(() => expect(within(dialog).queryByText("Итоги встречи по Raft")).toBeNull());
  });

  it("после «Удалить навсегда» число в строке прежнее, раскрытие говорит, сколько записей удалено", async () => {
    const api = setup();
    const kept = mockMessage({ id: "m1", chatId: "work", bodyMd: "Осталась", journalDate: "2026-03-14" });
    api.addAgentLog({ tool: "search", query: "Raft", at: AT, count: 3 }, [entry({ messageId: "m1", message: kept })]);
    api.addAgentLog({ tool: "add_entry", at: AT + 60_000, count: 1 }, []);
    const dialog = await openAgentDialog();
    const log = within(dialog).getByRole("region", { name: "Журнал обращений" });
    const rows = within(log).getAllByRole("listitem").filter((li) => li.className.includes("agent-log__row"));
    expect(rows[0]).toHaveTextContent(/добавить запись · 1 запись$/);
    expect(rows[1]).toHaveTextContent(/поиск «Raft» · 3 записи$/);
    fireEvent.click(within(rows[1] as HTMLElement).getByRole("button"));
    expect(await within(dialog).findByText("Осталась")).toBeInTheDocument();
    expect(within(dialog).getByText("ещё 2 записи удалены навсегда")).toBeInTheDocument();
    // Строка, у которой не осталось ни одной записи, всё равно раскрывается.
    fireEvent.click(within(rows[0] as HTMLElement).getByRole("button"));
    expect(await within(dialog).findByText("ещё 1 запись удалена навсегда")).toBeInTheDocument();
  });

  it("щелчок по записи открывает её в ленте: окна закрываются, чат выбран, запись найдена", async () => {
    const message = mockMessage({ id: "m1", chatId: "work", bodyMd: "Итоги встречи по Raft", journalDate: "2026-10-27", createdAt: Date.UTC(2026, 9, 27, 9, 0), origin: "agent" });
    const api = setup({ messages: [message] });
    api.addAgentLog({ tool: "add_entry", at: AT, count: 1 }, [entry({ messageId: "m1", message })]);
    const dialog = await openAgentDialog();
    fireEvent.click(within(dialog).getByRole("button", { name: /добавить запись/ }));
    fireEvent.click(await within(dialog).findByRole("button", { name: /Итоги встречи по Raft/ }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(await screen.findByRole("button", { name: /^Работа/ })).toHaveAttribute("aria-current", "true");
    const plate = await screen.findByText("Итоги встречи по Raft");
    expect(plate.closest("article")).toHaveTextContent("добавлено агентом");
  });

  it("запись удалена навсегда после чтения журнала: окно остаётся, на месте записи — «Запись удалена»", async () => {
    const orphan = mockMessage({ id: "ghost", chatId: "work", bodyMd: "Призрак", journalDate: "2026-10-27" });
    const api = setup();
    api.addAgentLog({ tool: "get_entry", at: AT, count: 1 }, [entry({ messageId: "ghost", message: orphan })]);
    const dialog = await openAgentDialog();
    fireEvent.click(within(dialog).getByRole("button", { name: /чтение записи/ }));
    fireEvent.click(await within(dialog).findByRole("button", { name: /Призрак/ }));
    await waitFor(() => expect(within(dialog).getByText("Запись удалена")).toBeInTheDocument());
    expect(within(dialog).queryByRole("button", { name: /Призрак/ })).toBeNull();
    expect(screen.getByRole("dialog", { name: "Доступ ИИ-агентов" })).toBeInTheDocument();
  });

  it("«Показать ещё» дочитывает старые строки страницами", async () => {
    const api = setup();
    for (let i = 0; i < 35; i += 1) api.addAgentLog({ tool: "list_chats", at: AT + i * 1000, count: 0 });
    const dialog = await openAgentDialog();
    const count = () => within(dialog).getAllByRole("listitem").filter((li) => li.className.includes("agent-log__row")).length;
    expect(count()).toBe(30);
    fireEvent.click(within(dialog).getByRole("button", { name: "Показать ещё" }));
    await waitFor(() => expect(count()).toBe(35));
    expect(within(dialog).queryByRole("button", { name: "Показать ещё" })).toBeNull();
  });

  it("журнал обновляется сам, пока окно открыто", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const api = setup();
      const dialog = await openAgentDialog();
      expect(within(dialog).getByText("Обращений не было")).toBeInTheDocument();
      api.addAgentLog({ tool: "list_chats", at: AT, count: 0 });
      await vi.advanceTimersByTimeAsync(2100);
      await waitFor(() => expect(within(dialog).getByText(/список чатов/)).toBeInTheDocument());
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("Пометка в ленте", () => {
  it("запись агента: «добавлено агентом» в строке времени; запись пользователя — без неё", async () => {
    setup({
      messages: [
        mockMessage({ id: "u", bodyMd: "Моя запись", createdAt: Date.UTC(2026, 9, 28, 9, 0), journalDate: "2026-10-28" }),
        mockMessage({ id: "a", bodyMd: "Запись агента", createdAt: Date.UTC(2026, 9, 28, 9, 5), journalDate: "2026-10-28", origin: "agent" }),
      ],
    });
    const agent = (await screen.findByText("Запись агента")).closest("article");
    const mine = (await screen.findByText("Моя запись")).closest("article");
    expect(agent).toHaveTextContent("добавлено агентом");
    expect(agent?.querySelector(".message__origin")).not.toBeNull();
    expect(mine).not.toHaveTextContent("добавлено агентом");
  });
});
