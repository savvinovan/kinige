import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "../../test/render";
import type { SidebarChat } from "./chats";
import { type ChatAction, type ChatTint, Sidebar } from "./Sidebar";

// KINIG-59: the "Colour" submenu of the sidebar chat menu.
const chat = (id: string, name: string, extra: Partial<SidebarChat> = {}): SidebarChat => ({ id, name, emoji: "💬", tint: "teal", ...extra });

const LIST: SidebarChat[] = [
  chat("inbox", "", { isInbox: true, emoji: "📥", tint: "grey" }),
  chat("projects", "Проекты", { tint: "pink" }),
  chat("books", "Книги", { emoji: "📚" }),
  chat("old", "Старый проект", { archived: true }),
];

const menu = () => screen.getByRole("menu");
const names = () => [...menu().querySelectorAll('[role^="menuitem"]')].map((item) => item.textContent);

function setup(props: Partial<React.ComponentProps<typeof Sidebar>> = {}, language: "ru" | "en" = "ru") {
  const onChatAction = vi.fn<(chatId: string, action: ChatAction) => void>();
  const onChatColor = vi.fn<(chatId: string, color: ChatTint) => void>();
  const view = render(<Sidebar chats={LIST} selectedId="projects" showSearch={false} onChatAction={onChatAction} onChatColor={onChatColor} {...props} />, { language });
  return { onChatAction, onChatColor, unmount: view.unmount };
}

const openMenu = (name: RegExp) => fireEvent.contextMenu(screen.getByRole("button", { name }));
const openColors = () => fireEvent.click(within(menu()).getByRole("menuitem", { name: /Цвет|Colour/ }));

describe("пункт «Цвет»: когда он есть", () => {
  it("у чата с буквенной плиткой стоит после «Сменить эмодзи», с шевроном и значком палитры", () => {
    setup();
    openMenu(/^Проекты/);
    expect(names()).toEqual(["Переименовать", "Сменить эмодзи", "Цвет", "Закрепить", "В архив"]);
    const item = within(menu()).getByRole("menuitem", { name: "Цвет" });
    expect(item).toHaveAttribute("aria-haspopup", "menu");
    expect(item.querySelectorAll("svg")).toHaveLength(2);
  });

  it("у чата с настоящим эмодзи, у Inbox и без обработчика пункта нет", () => {
    const { unmount } = setup();
    openMenu(/^Книги/);
    expect(names()).toEqual(["Переименовать", "Сменить эмодзи", "Закрепить", "В архив"]);
    fireEvent.keyDown(menu(), { key: "Escape" });
    openMenu(/^Входящие/);
    expect(names()).toEqual(["Закрепить"]);
    unmount();
    setup({ onChatColor: undefined });
    openMenu(/^Проекты/);
    expect(names()).toEqual(["Переименовать", "Сменить эмодзи", "Закрепить", "В архив"]);
  });

  it("у архивного чата с буквенной плиткой пункт есть", () => {
    setup();
    fireEvent.click(screen.getByRole("button", { name: "Архив" }));
    openMenu(/^Старый проект/);
    expect(names()).toEqual(["Переименовать", "Сменить эмодзи", "Цвет", "Из архива"]);
  });
});

describe("подменю «Цвет»", () => {
  it("«Назад» и семь строк: буква чата на плитке каждого цвета, название цвета, галочка у текущего", () => {
    setup();
    openMenu(/^Проекты/);
    openColors();
    expect(menu()).toHaveAccessibleName("Цвет чата: выберите цвет");
    expect(names()).toEqual(["Назад", "ПСиний", "ПБирюзовый", "ПЗелёный", "ПОранжевый", "ПРозовый", "ПФиолетовый", "ПСерый"]);
    const radios = within(menu()).getAllByRole("menuitemradio");
    expect(radios).toHaveLength(7);
    expect(radios.map((radio) => radio.querySelector(".chat-tile")?.className.replace("chat-tile chat-tile--", ""))).toEqual([
      "blue",
      "teal",
      "green",
      "orange",
      "pink",
      "violet",
      "grey",
    ]);
    expect(radios.map((radio) => radio.getAttribute("aria-checked"))).toEqual(["false", "false", "false", "false", "true", "false", "false"]);
    expect(radios[4]?.querySelector(".context-menu__check svg")).not.toBeNull();
    expect(radios[0]?.querySelector(".context-menu__check")).toBeNull();
  });

  it("выбор цвета зовёт onChatColor(id, цвет) и закрывает меню", () => {
    const { onChatColor, onChatAction } = setup();
    openMenu(/^Проекты/);
    openColors();
    fireEvent.click(within(menu()).getByRole("menuitemradio", { name: /Серый/ }));
    expect(onChatColor).toHaveBeenCalledExactlyOnceWith("projects", "grey");
    expect(onChatAction).not.toHaveBeenCalled();
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("повторный выбор текущего цвета тоже сообщается (в Rust это пустая запись, не ошибка)", () => {
    const { onChatColor } = setup();
    openMenu(/^Проекты/);
    openColors();
    fireEvent.click(within(menu()).getByRole("menuitemradio", { name: /Розовый/ }));
    expect(onChatColor).toHaveBeenCalledWith("projects", "pink");
  });

  it("с клавиатуры: Enter открывает подменю, фокус на «Назад»; влево и Esc возвращают, второй Esc закрывает", () => {
    setup();
    openMenu(/^Проекты/);
    for (let i = 0; i < 2; i++) fireEvent.keyDown(menu(), { key: "ArrowDown" });
    const item = within(menu()).getByRole("menuitem", { name: "Цвет" });
    expect(item).toHaveFocus();
    fireEvent.click(item);
    expect(within(menu()).getByRole("menuitem", { name: "Назад" })).toHaveFocus();
    fireEvent.keyDown(menu(), { key: "ArrowDown" });
    expect(within(menu()).getByRole("menuitemradio", { name: /Синий/ })).toHaveFocus();
    fireEvent.keyDown(menu(), { key: "ArrowLeft" });
    expect(names()).toContain("Переименовать");
    openColors();
    fireEvent.keyDown(menu(), { key: "Escape" });
    expect(names()).toContain("Переименовать");
    fireEvent.keyDown(menu(), { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("«Назад» возвращает на первый уровень без выбора", () => {
    const { onChatColor } = setup();
    openMenu(/^Проекты/);
    openColors();
    fireEvent.click(within(menu()).getByRole("menuitem", { name: "Назад" }));
    expect(names()).toContain("Закрепить");
    expect(onChatColor).not.toHaveBeenCalled();
  });

  it("на английском: Colour, Back и названия цветов", () => {
    setup({}, "en");
    openMenu(/^Проекты/);
    openColors();
    expect(menu()).toHaveAccessibleName("Chat colour: choose a colour");
    expect(names()).toEqual(["Back", "ПBlue", "ПTeal", "ПGreen", "ПOrange", "ПPink", "ПViolet", "ПGrey"]);
  });

  it("в свёрнутом сайдбаре меню то же", () => {
    setup({ collapsed: true, onToggleCollapsed: () => {}, onSelect: () => {} });
    fireEvent.contextMenu(screen.getByRole("button", { name: "Проекты" }));
    expect(names()).toContain("Цвет");
  });
});
