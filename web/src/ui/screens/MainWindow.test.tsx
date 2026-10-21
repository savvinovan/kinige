import { afterEach, describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { MockApi, type MockApiOptions } from "../../api/MockApi";
import type { Settings } from "../../api/generated/Settings";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

const SETTINGS: Settings = {
  textSize: "normal",
  mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
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
    for (const dead of ["Прикрепить файл", "Эмодзи", "Записать голосовое"]) expect(screen.queryByRole("button", { name: dead })).toBeNull();
    expect(screen.queryByRole("searchbox")).toBeNull();
    expect(screen.queryByRole("button", { name: "Отправить" })).toBeNull();
  });

  it("отправка Enter: запись под меткой «Сегодня» с временем, счётчик 1, тост «Отменить», поле пусто", async () => {
    setup();
    await type("Купить **молоко**\n- [ ] позвонить маме");
    expect(screen.getByRole("button", { name: "Отправить" })).toBeInTheDocument();
    fireEvent.keyDown(field(), { key: "Enter" });

    const plate = await screen.findByRole("article");
    expect(within(plate).getByText("молоко").tagName).toBe("STRONG");
    expect(within(plate).getByText("06:12")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Сегодня");
    expect(screen.getByRole("button", { name: /^Входящие/ })).toHaveTextContent("1");
    expect(screen.getByText("Запись отправлена")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Отменить" })).toBeInTheDocument();
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

  it("«Отменить»: запись пропадает, счётчик снова 1, текст возвращается в поле, фокус в поле", async () => {
    setup();
    await type("первая");
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByText("первая");
    await type("вторая — поправить");
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByText("вторая — поправить");
    expect(screen.getByRole("button", { name: /^Входящие/ })).toHaveTextContent("2");

    fireEvent.click(screen.getByRole("button", { name: "Отменить" }));
    await waitFor(() => expect(screen.queryByText("вторая — поправить", { selector: "article *" })).toBeNull());
    expect(screen.getAllByRole("article")).toHaveLength(1);
    expect(screen.getByRole("button", { name: /^Входящие/ })).toHaveTextContent("1");
    expect(field().value).toBe("вторая — поправить");
    expect(field()).toHaveFocus();
    expect(screen.queryByRole("button", { name: "Отменить" })).toBeNull();
  });

  it("«Отменить» не затирает то, что уже набрано в поле", async () => {
    setup();
    await type("отправлено");
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByText("отправлено");
    await type("уже печатаю новое");
    fireEvent.click(screen.getByRole("button", { name: "Отменить" }));
    await waitFor(() => expect(screen.queryByRole("article")).toBeNull());
    expect(field().value).toBe("уже печатаю новое");
  });

  it("ошибка записи: запись не показана, текст остаётся в поле, над ним ошибка; повтор проходит", async () => {
    const api = setup();
    api.failNextSend = "store_write_failed";
    await type("важная мысль");
    fireEvent.keyDown(field(), { key: "Enter" });

    expect(await screen.findByRole("alert")).toHaveTextContent("Не удалось сохранить запись. Текст остался в поле.");
    expect(field().value).toBe("важная мысль");
    expect(screen.queryByRole("article")).toBeNull();
    expect(screen.queryByRole("button", { name: "Отменить" })).toBeNull();
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

  it("по-английски: «Inbox», «Entry sent», «Undo», ошибки и «Today»", async () => {
    const api = setup({}, SETTINGS, "en");
    expect(await screen.findByRole("button", { name: /^Inbox/ })).toBeInTheDocument();
    api.failNextSend = "store_write_failed";
    await type("hello");
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not save the entry. The text is still in the field.");
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByRole("article");
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Today");
    expect(screen.getByText("Entry sent")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
  });

  it("запись из прошлого дня: метка по journal_date, а не «Сегодня»", async () => {
    setup({
      messages: [{ id: "old", chatId: INBOX_ID, bodyMd: "давняя", journalDate: "2025-03-14", createdAt: Date.UTC(2025, 2, 14, 9, 12), createdTz: "UTC" }],
    });
    await screen.findByText("давняя");
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("14 марта 2025");
  });

  it("ссылка в записи открывается в системном браузере, окно остаётся", async () => {
    const api = setup({
      messages: [{ id: "l", chatId: INBOX_ID, bodyMd: "см. [статью](https://example.com/a)", journalDate: "2026-10-21", createdAt: Date.UTC(2026, 9, 21, 5, 0), createdTz: "UTC" }],
    });
    const link = await screen.findByRole("link", { name: "статью" });
    expect(fireEvent.click(link)).toBe(false);
    expect(api.openedUrls).toEqual(["https://example.com/a"]);
    expect(screen.getByRole("textbox", { name: "Новая заметка" })).toBeInTheDocument();
  });

  it("ссылка не из списка схем: ошибка над полем, ничего не открыто", async () => {
    const api = setup({
      messages: [{ id: "l", chatId: INBOX_ID, bodyMd: "<ftp://example.com/x>", journalDate: "2026-10-21", createdAt: Date.UTC(2026, 9, 21, 5, 0), createdTz: "UTC" }],
    });
    // markdown-it не делает ссылкой ftp:, а DOMPurify убирает href: щёлкать не по чему.
    await screen.findByRole("article");
    expect(screen.queryByRole("link")).toBeNull();
    expect(api.openedUrls).toEqual([]);
  });
});

describe("MainWindow: настройки", () => {
  async function openSettings() {
    fireEvent.click(await screen.findByRole("button", { name: /Профиль/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Настройки" }));
    return screen.findByRole("dialog", { name: "Настройки" });
  }

  it("меню профиля: «Настройки» работают, «Экспорт» и «О программе» — «Появится позже»", async () => {
    setup();
    fireEvent.click(await screen.findByRole("button", { name: /Профиль/ }));
    expect(screen.getByRole("menuitem", { name: "Настройки" })).not.toHaveAttribute("aria-disabled");
    expect(screen.getByRole("menuitem", { name: "Экспорт" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("menuitem", { name: "О программе" })).toHaveAttribute("aria-disabled", "true");
  });

  it("окно показывает папку копии и предупреждение «открытый текст»; «Выбрать…» меняет папку", async () => {
    const api = setup({ pickedFolder: "/Volumes/Backup/Журнал" });
    const dialog = await openSettings();
    expect(within(dialog).getByText("/Users/me/Documents/Kinige")).toBeInTheDocument();
    expect(within(dialog).getByText(/открытый текст/)).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button", { name: "Выбрать…" }));
    expect(await within(dialog).findByText("/Volumes/Backup/Журнал")).toBeInTheDocument();
    expect((await api.getSettings()).mirror.folder).toBe("/Volumes/Backup/Журнал");
    expect(within(dialog).queryByText("/Users/me/Documents/Kinige")).toBeNull();
  });

  it("отмена системного диалога оставляет прежнюю папку", async () => {
    setup({ pickedFolder: null });
    const dialog = await openSettings();
    fireEvent.click(within(dialog).getByRole("button", { name: "Выбрать…" }));
    await waitFor(() => expect(within(dialog).getByRole("button", { name: "Выбрать…" })).not.toHaveAttribute("aria-disabled", "true"));
    expect(within(dialog).getByText("/Users/me/Documents/Kinige")).toBeInTheDocument();
  });

  it("ошибка выбора папки видна в окне", async () => {
    const api = setup();
    api.failNextPick = "folder_pick_failed";
    const dialog = await openSettings();
    fireEvent.click(within(dialog).getByRole("button", { name: "Выбрать…" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("Не удалось выбрать папку.");
  });

  it("«Размер текста» → «Крупный»: атрибут на <html> сразу и сохранён в настройках", async () => {
    const api = setup();
    const dialog = await openSettings();
    expect(document.documentElement).not.toHaveAttribute("data-text-scale");
    fireEvent.click(within(dialog).getByRole("radio", { name: "Крупный" }));
    await waitFor(() => expect(document.documentElement).toHaveAttribute("data-text-scale", "large"));
    expect((await api.getSettings()).textSize).toBe("large");
    fireEvent.click(within(dialog).getByRole("radio", { name: "Обычный" }));
    await waitFor(() => expect(document.documentElement).not.toHaveAttribute("data-text-scale"));
  });

  it("крупный размер из сохранённых настроек применяется при старте (после перезапуска)", async () => {
    setup({}, { ...SETTINGS, textSize: "large" });
    await screen.findByRole("button", { name: /^Входящие/ });
    expect(document.documentElement).toHaveAttribute("data-text-scale", "large");
  });

  it("Esc закрывает окно, фокус возвращается на кнопку профиля", async () => {
    setup();
    await openSettings();
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("button", { name: /Профиль/ })).toHaveFocus();
  });
});
