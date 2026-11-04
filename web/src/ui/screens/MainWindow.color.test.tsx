import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import { automaticColor } from "../../api/mockColor";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// KINIG-59 on the real `MainWindow` and `MockApi`: pick a colour in the chat menu, see it in the sidebar at once, in search rows, in the
// trash plate and in the "Move to chat" submenu; a new chat gets an automatic colour; an error comes back above the composer.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const TODAY_AT = Date.UTC(2026, 9, 21, 5, 0);

function setup() {
  const api = new MockApi({
    now: NOW,
    chats: [
      { id: "projects", title: "Проекты", icon: "💬" },
      { id: "books", title: "Книги", icon: "📚" },
    ],
    messages: [
      mockMessage({ id: "m1", chatId: "projects", bodyMd: "Уникальноеслово в проектах", createdAt: TODAY_AT }),
      mockMessage({ id: "m2", chatId: "projects", bodyMd: "Эту запись удалим", createdAt: TODAY_AT - 1000 }),
    ],
  });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
  return api;
}

const row = (name: RegExp) => screen.getByRole("button", { name });
const tileIn = (element: HTMLElement) => element.querySelector(".chat-tile");
const menu = () => screen.getByRole("menu");

async function pickColor(chat: RegExp, color: RegExp) {
  fireEvent.contextMenu(await screen.findByRole("button", { name: chat }));
  fireEvent.click(within(menu()).getByRole("menuitem", { name: "Цвет" }));
  fireEvent.click(within(menu()).getByRole("menuitemradio", { name: color }));
}

describe("цвет чата: меню, сайдбар, поиск, корзина, перенос", () => {
  it("до выбора плитка имеет автоматический цвет, у Входящих серая", async () => {
    setup();
    const projects = await screen.findByRole("button", { name: /^Проекты/ });
    expect(tileIn(projects)).toHaveClass(`chat-tile--${automaticColor("projects")}`);
    expect(tileIn(row(/^Входящие/))).toBeNull();
    expect(row(/^Книги/).querySelector(".chat-emoji")).not.toBeNull();
  });

  it("выбор «Серый»: плитка в сайдбаре серая сразу, без тоста и сообщений; цвет хранится в данных", async () => {
    const api = setup();
    await pickColor(/^Проекты/, /Серый/);
    await waitFor(() => expect(tileIn(row(/^Проекты/))).toHaveClass("chat-tile--grey"));
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
    expect((await api.listChats()).chats.find((chat) => chat.id === "projects")?.color).toBe("grey");
  });

  it("поиск: строка чата и строки записей несут плитку выбранного цвета", async () => {
    setup();
    await pickColor(/^Проекты/, /Фиолетовый/);
    await waitFor(() => expect(tileIn(row(/^Проекты/))).toHaveClass("chat-tile--violet"));
    fireEvent.change(screen.getByRole("combobox", { name: "Поиск" }), { target: { value: "Проект" } });
    const results = await screen.findByRole("region", { name: "Результаты поиска" });
    await waitFor(() => expect(within(results).getAllByRole("option").length).toBeGreaterThan(0));
    const chatRow = results.querySelector(".search-chat");
    expect(tileIn(chatRow as HTMLElement)).toHaveClass("chat-tile--violet");

    fireEvent.change(screen.getByRole("combobox", { name: "Поиск" }), { target: { value: "Уникальноеслово" } });
    await waitFor(() => expect(results.querySelector(".search-hit")).not.toBeNull());
    expect(tileIn(results.querySelector(".search-hit") as HTMLElement)).toHaveClass("chat-tile--violet");
  });

  it("корзина: плашка удалённой записи несёт плитку чата", async () => {
    const api = setup();
    await pickColor(/^Проекты/, /Оранжевый/);
    await api.deleteMessage("m2");
    fireEvent.click(await screen.findByRole("button", { name: /^Корзина/ }));
    const region = await screen.findByRole("region", { name: "Корзина" });
    const plate = await within(region).findByRole("article");
    expect(tileIn(plate)).toHaveClass("chat-tile--orange");
    expect(plate).toHaveTextContent("Проекты");
  });

  it("«Переместить в чат»: чаты с буквой — плитка своего цвета, чаты с эмодзи и Входящие — эмодзи", async () => {
    setup();
    await pickColor(/^Проекты/, /Зелёный/);
    await waitFor(() => expect(tileIn(row(/^Проекты/))).toHaveClass("chat-tile--green"));
    fireEvent.click(row(/^Книги/));
    fireEvent.click(row(/^Проекты/));
    fireEvent.contextMenu((await screen.findByText("Уникальноеслово в проектах")).closest("article") as HTMLElement);
    fireEvent.click(within(menu()).getByRole("menuitem", { name: "Переместить в чат" }));
    const projects = within(menu()).getByRole("menuitem", { name: /Проекты/ });
    expect(tileIn(projects)).toHaveClass("chat-tile--green");
    expect(tileIn(projects)).toHaveTextContent("П");
    expect(tileIn(within(menu()).getByRole("menuitem", { name: /Книги/ }))).toBeNull();
    expect(within(menu()).getByRole("menuitem", { name: /Книги/ }).querySelector(".chat-emoji")).toHaveTextContent("📚");
    expect(within(menu()).getByRole("menuitem", { name: /Входящие/ }).querySelector(".chat-emoji")).toHaveTextContent("📥");
  });

  it("новый чат без эмодзи получает автоматический цвет сразу", async () => {
    const api = setup();
    await screen.findByRole("button", { name: /^Проекты/ });
    const created = await api.createChat("Дела", null);
    api.emitEntriesChanged();
    const tile = await waitFor(() => {
      const found = tileIn(row(/^Дела/));
      expect(found).not.toBeNull();
      return found;
    });
    expect(tile).toHaveClass(`chat-tile--${automaticColor(created.id)}`);
  });

  it("ошибка записи: строка ошибки над полем, плитка не меняется", async () => {
    const api = setup();
    const before = automaticColor("projects");
    api.failNext("setChatColor", "read_only");
    await pickColor(/^Проекты/, /Серый/);
    expect(await screen.findByRole("alert")).toHaveTextContent(/более новой версией/);
    expect(tileIn(row(/^Проекты/))).toHaveClass(`chat-tile--${before}`);
  });
});
