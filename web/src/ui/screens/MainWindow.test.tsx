import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, MockApi, type MockApiOptions, mockMessage } from "../../api/MockApi";
import type { Settings } from "../../api/generated/Settings";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { DEFAULT_SHORTCUTS } from "../settings/demoShortcuts";
import { MainWindow } from "./MainWindow";

const SETTINGS: Settings = {
  ...DEFAULT_SETTINGS,
  mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
  shortcuts: DEFAULT_SHORTCUTS,
};
// 2026-10-21 06:12 UTC; запись создаётся в поясе UTC, время в ленте — «06:12».
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

function setup(options: MockApiOptions = {}, settings: Settings = SETTINGS, language: "ru" | "en" = "ru") {
  const api = new MockApi({ now: NOW, ...options });
  render(<MainWindow api={api} initialSettings={settings} />, { language });
  return api;
}

const field = () => screen.getByRole("textbox", { name: /Новая заметка|New note/ }) as HTMLTextAreaElement;

async function type(text: string) {
  fireEvent.change(await screen.findByRole("textbox", { name: /Новая заметка|New note/ }), { target: { value: text } });
}

afterEach(() => document.documentElement.removeAttribute("data-text-scale"));

describe("MainWindow на MockApi: сценарий слайса 1", () => {
  it("первый запуск: один чат Inbox без счётчика, пустая лента, курсор в поле ввода, без мёртвых кнопок и поиска", async () => {
    setup();
    expect(await screen.findByRole("button", { name: /^Входящие/ })).toHaveAttribute("aria-current", "true");
    expect(screen.getAllByRole("button", { name: /^Входящие/ })).toHaveLength(1);
    expect(document.querySelector(".chat-row__count")).toBeNull();
    expect(screen.getByText(/Пока пусто/)).toBeInTheDocument();
    expect(field()).toHaveFocus();
    for (const dead of ["Прикрепить файл", "Записать голосовое"]) expect(screen.queryByRole("button", { name: dead })).toBeNull();
    expect(screen.queryByRole("searchbox")).toBeNull();
    expect(screen.queryByRole("button", { name: "Отправить" })).toBeNull();
  });

  it("отправка Enter: запись под меткой «Сегодня» с временем, счётчик 1, поле пусто, никакого тоста и подтверждения", async () => {
    setup();
    await type("Купить **молоко**\n- [ ] позвонить маме");
    expect(screen.getByRole("button", { name: "Отправить" })).toBeInTheDocument();
    fireEvent.keyDown(field(), { key: "Enter" });

    const plate = await screen.findByRole("article");
    expect(within(plate).getByText("молоко").tagName).toBe("STRONG");
    expect(within(plate).getByText("06:12")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Сегодня");
    expect(screen.getByRole("button", { name: /^Входящие/ })).toHaveTextContent("1");
    // После отправки нет ни тоста, ни другого подтверждения: запись просто появилась в ленте.
    expect(screen.queryByText("Запись отправлена")).toBeNull();
    expect(screen.queryByRole("button", { name: "Отменить" })).toBeNull();
    expect(document.querySelector(".toast, .toast-region")).toBeNull();
    expect(field().value).toBe("");
  });

  it("Shift+Enter — перенос строки, ничего не отправляется", async () => {
    setup();
    await type("первая строка");
    fireEvent.keyDown(field(), { key: "Enter", shiftKey: true });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.queryByRole("article")).toBeNull();
    expect(field().value).toBe("первая строка");
  });

  it("кнопка «Отправить» делает то же, что Enter", async () => {
    setup();
    await type("через кнопку");
    fireEvent.click(screen.getByRole("button", { name: "Отправить" }));
    expect(await screen.findByRole("article")).toHaveTextContent("через кнопку");
  });

  it("ошибка записи: запись не показана, текст остаётся в поле, над ним ошибка; повтор проходит", async () => {
    const api = setup();
    api.failNextSend = "store_write_failed";
    await type("важная мысль");
    fireEvent.keyDown(field(), { key: "Enter" });

    expect(await screen.findByRole("alert")).toHaveTextContent("Не удалось сохранить запись. Текст остался в поле.");
    expect(field().value).toBe("важная мысль");
    expect(screen.queryByRole("article")).toBeNull();
    expect(document.querySelector(".chat-row__count")).toBeNull();

    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await screen.findByRole("article")).toHaveTextContent("важная мысль");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("ошибка исчезает, как только человек правит текст", async () => {
    const api = setup();
    api.failNextSend = "store_write_failed";
    await type("текст");
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByRole("alert");
    await type("текст!");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("по-английски: «Inbox», ошибки и «Today»", async () => {
    const api = setup({}, SETTINGS, "en");
    expect(await screen.findByRole("button", { name: /^Inbox/ })).toBeInTheDocument();
    api.failNextSend = "store_write_failed";
    await type("hello");
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not save the entry. The text is still in the field.");
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByRole("article");
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Today");
  });

  it("запись из прошлого дня: метка по journal_date, а не «Сегодня»", async () => {
    setup({
      messages: [mockMessage({ id: "old", bodyMd: "давняя", journalDate: "2025-03-14", createdAt: Date.UTC(2025, 2, 14, 9, 12) })],
    });
    await screen.findByText("давняя");
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("14 марта 2025");
  });

  it("ссылка в записи открывается в системном браузере, окно остаётся", async () => {
    const api = setup({
      messages: [mockMessage({ id: "l", bodyMd: "см. [статью](https://example.com/a)", createdAt: Date.UTC(2026, 9, 21, 5, 0) })],
    });
    const link = await screen.findByRole("link", { name: "статью" });
    expect(fireEvent.click(link)).toBe(false);
    expect(api.openedUrls).toEqual(["https://example.com/a"]);
    expect(screen.getByRole("textbox", { name: "Новая заметка" })).toBeInTheDocument();
  });

  it("ссылка не из списка схем: ошибка над полем, ничего не открыто", async () => {
    const api = setup({
      messages: [mockMessage({ id: "l", bodyMd: "<ftp://example.com/x>", createdAt: Date.UTC(2026, 9, 21, 5, 0) })],
    });
    // markdown-it не делает ссылкой ftp:, а DOMPurify убирает href: щёлкать не по чему.
    await screen.findByRole("article");
    expect(screen.queryByRole("link")).toBeNull();
    expect(api.openedUrls).toEqual([]);
  });
});

describe("MainWindow: настройки живут в отдельном окне (слайс 4b)", () => {
  async function chooseSettingsInMenu() {
    fireEvent.click(await screen.findByRole("button", { name: /Профиль/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Настройки" }));
  }

  it("меню профиля: «Настройки» работают, «Экспорт» и «О программе» — «Появится позже»", async () => {
    setup();
    fireEvent.click(await screen.findByRole("button", { name: /Профиль/ }));
    expect(screen.getByRole("menuitem", { name: "Настройки" })).not.toHaveAttribute("aria-disabled");
    expect(screen.getByRole("menuitem", { name: "Экспорт" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("menuitem", { name: "О программе" })).toHaveAttribute("aria-disabled", "true");
  });

  it("«Настройки» просят у Rust открыть отдельное окно; в главном окне диалога нет", async () => {
    const api = setup();
    await chooseSettingsInMenu();
    await waitFor(() => expect(api.openSettingsCount).toBe(1));
    expect(screen.queryByRole("dialog")).toBeNull();
    // Повторный выбор — снова просьба (Rust поднимет то же окно), диалога в этом окне по-прежнему нет.
    await chooseSettingsInMenu();
    await waitFor(() => expect(api.openSettingsCount).toBe(2));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("событие «настройки изменились»: размер текста применяется сразу, без перезапуска", async () => {
    const api = setup();
    await screen.findByRole("button", { name: /^Входящие/ });
    expect(document.documentElement).not.toHaveAttribute("data-text-scale");
    await api.setTextSize("large");
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-text-scale", "large"));
    await api.setTextSize("normal");
    await waitFor(() => expect(document.documentElement).not.toHaveAttribute("data-text-scale"));
  });

  it("после закрытия окна подписка на «настройки изменились» снята", async () => {
    const api = new MockApi({ now: NOW });
    const off = vi.fn();
    vi.spyOn(api, "onSettingsChanged").mockResolvedValue(off);
    const { unmount } = render(<MainWindow api={api} initialSettings={SETTINGS} />);
    await screen.findByRole("button", { name: /^Входящие/ });
    await waitFor(() => expect(api.onSettingsChanged).toHaveBeenCalled());
    unmount();
    expect(off).toHaveBeenCalled();
  });

  it("запись из журнала обращений (окно настроек) открывается в ленте этого окна", async () => {
    const api = setup({
      chats: [{ id: "work", title: "Работа", icon: "💼" }],
      messages: [mockMessage({ id: "a", chatId: "work", bodyMd: "запись из чата Работа", createdAt: Date.UTC(2026, 9, 21, 5, 0) })],
    });
    await screen.findByRole("button", { name: /^Входящие/ });
    await api.settingsOpenEntry("a");
    expect(await screen.findByText("запись из чата Работа")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Работа/ })).toHaveAttribute("aria-current", "true");
  });

  it("крупный размер из сохранённых настроек применяется при старте (после перезапуска)", async () => {
    setup({}, { ...SETTINGS, textSize: "large" });
    await screen.findByRole("button", { name: /^Входящие/ });
    expect(document.documentElement).toHaveAttribute("data-text-scale", "large");
  });
});
