import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "../../test/render";
import { chatMenuItems, type SidebarChat } from "./chats";
import { Sidebar, type ChatAction } from "./Sidebar";
import { createI18n } from "../../i18n";

const chat = (id: string, name: string, extra: Partial<SidebarChat> = {}): SidebarChat => ({ id, name, emoji: "📌", ...extra });

const LIST: SidebarChat[] = [
  chat("inbox", "", { isInbox: true, emoji: "📥", count: 3 }),
  chat("fav", "Избранное", { pinned: true }),
  chat("books", "Книги", { count: 12 }),
  chat("old", "Старый проект", { archived: true }),
];

const menu = () => screen.getByRole("menu");
const itemNames = () => within(menu()).getAllByRole("menuitem").map((item) => item.textContent);

const setup = (props: Partial<React.ComponentProps<typeof Sidebar>> = {}) => {
  const onChatAction = vi.fn<(chatId: string, action: ChatAction) => void>();
  const view = render(<Sidebar chats={LIST} selectedId="books" showSearch={false} onChatAction={onChatAction} {...props} />);
  return { onChatAction, ...view };
};

describe("контекстное меню чата", () => {
  it("правый щелчок: переименовать, сменить эмодзи, закрепить, в архив", () => {
    setup();
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Книги/ }), { clientX: 20, clientY: 30 });
    expect(menu()).toHaveAccessibleName("Действия с чатом «Книги»");
    expect(itemNames()).toEqual(["Переименовать", "Сменить эмодзи", "Закрепить", "В архив"]);
  });

  it("закреплённый чат: «Открепить»; архивный: «Из архива» и без закрепления", () => {
    setup();
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Избранное/ }));
    expect(itemNames()).toEqual(["Переименовать", "Сменить эмодзи", "Открепить", "В архив"]);
    fireEvent.keyDown(menu(), { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "Архив" }));
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Старый проект/ }));
    expect(itemNames()).toEqual(["Переименовать", "Сменить эмодзи", "Из архива"]);
  });

  it("у Inbox только «Закрепить» / «Открепить»; название в подписи меню переведено", () => {
    const { unmount } = setup();
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Входящие/ }));
    expect(itemNames()).toEqual(["Закрепить"]);
    expect(menu()).toHaveAccessibleName("Действия с чатом «Входящие»");
    unmount();
    setup({ chats: [chat("inbox", "", { isInbox: true, pinned: true })] });
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Входящие/ }));
    expect(itemNames()).toEqual(["Открепить"]);
  });

  it("выбор пункта вызывает onChatAction(id, действие) и закрывает меню", () => {
    const { onChatAction } = setup();
    const expected: [string, ChatAction][] = [
      ["Переименовать", "rename"],
      ["Сменить эмодзи", "icon"],
      ["Закрепить", "pin"],
      ["В архив", "archive"],
    ];
    for (const [label, action] of expected) {
      fireEvent.contextMenu(screen.getByRole("button", { name: /^Книги/ }));
      fireEvent.click(within(menu()).getByRole("menuitem", { name: label }));
      expect(onChatAction).toHaveBeenLastCalledWith("books", action);
      expect(screen.queryByRole("menu")).toBeNull();
    }
    expect(onChatAction).toHaveBeenCalledTimes(4);
  });

  it("клавиатура: Shift+F10 и клавиша меню открывают меню, стрелки ходят, Enter выбирает, Esc закрывает и возвращает фокус", () => {
    const { onChatAction } = setup();
    const row = screen.getByRole("button", { name: /^Книги/ });
    row.focus();
    fireEvent.keyDown(row, { key: "F10", shiftKey: true });
    expect(menu()).toBeInTheDocument();
    expect(within(menu()).getByRole("menuitem", { name: "Переименовать" })).toHaveFocus();
    fireEvent.keyDown(menu(), { key: "ArrowDown" });
    expect(within(menu()).getByRole("menuitem", { name: "Сменить эмодзи" })).toHaveFocus();
    fireEvent.keyDown(menu(), { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    expect(row).toHaveFocus();
    fireEvent.keyDown(row, { key: "ContextMenu" });
    fireEvent.keyDown(menu(), { key: "End" });
    expect(within(menu()).getByRole("menuitem", { name: "В архив" })).toHaveFocus();
    fireEvent.click(document.activeElement as HTMLElement);
    expect(onChatAction).toHaveBeenCalledExactlyOnceWith("books", "archive");
  });

  it("без onChatAction меню у строк нет", () => {
    render(<Sidebar chats={LIST} selectedId="books" showSearch={false} />);
    const row = screen.getByRole("button", { name: /^Книги/ });
    expect(row).not.toHaveAttribute("aria-haspopup");
    fireEvent.contextMenu(row);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("английский: пункты и подпись", () => {
    render(<Sidebar chats={LIST} selectedId="books" showSearch={false} onChatAction={() => {}} />, { language: "en" });
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Книги/ }));
    expect(menu()).toHaveAccessibleName("Actions for chat “Книги”");
    expect(itemNames()).toEqual(["Rename", "Change emoji", "Pin", "Archive"]);
  });

  it("chatMenuItems: опасных пунктов нет, архив отделён чертой", () => {
    const t = createI18n("ru").t;
    const items = chatMenuItems(chat("a", "А"), t);
    expect(items.map((item) => item.id)).toEqual(["rename", "icon", "pin", "archive"]);
    expect(items.some((item) => item.danger)).toBe(false);
    expect(items.at(-1)?.separatorBefore).toBe(true);
  });
});

describe("свёрнутый сайдбар: полоса эмодзи", () => {
  const rail = (props: Partial<React.ComponentProps<typeof Sidebar>> = {}) =>
    setup({ collapsed: true, onToggleCollapsed: () => {}, onSelect: () => {}, ...props });

  it("только эмодзи: нет поиска, заголовков и имён в тексте; аside помечен как полоса", () => {
    rail();
    expect(document.querySelector("aside")).toHaveClass("sidebar--rail");
    expect(screen.queryByRole("searchbox")).toBeNull();
    expect(screen.queryByText("Закреплённые")).toBeNull();
    expect(screen.queryByText("Книги")).toBeNull();
  });

  it("кнопки чатов подписаны именем (и числом записей), эмодзи — декор; архивных чатов нет", () => {
    rail();
    const nav = screen.getByRole("navigation", { name: "Чаты" });
    const buttons = within(nav).getAllByRole("button");
    expect(buttons.map((button) => button.getAttribute("aria-label"))).toEqual(["Избранное", "Входящие, 3 записи", "Книги, 12 записей"]);
    expect(within(nav).queryByRole("button", { name: /Старый проект/ })).toBeNull();
    expect(buttons[1]?.textContent).toBe("📥");
  });

  it("закреплённые идут первыми, выбранный чат подсвечен и помечен aria-current", () => {
    rail({ selectedId: "fav" });
    const buttons = within(screen.getByRole("navigation", { name: "Чаты" })).getAllByRole("button");
    expect(buttons.map((button) => button.getAttribute("aria-current"))).toEqual(["true", null, null]);
    expect(buttons[0]).toHaveClass("rail-button--selected");
  });

  it("щелчок выбирает чат; «развернуть» вызывает onToggleCollapsed", () => {
    const onSelect = vi.fn();
    const onToggleCollapsed = vi.fn();
    rail({ onSelect, onToggleCollapsed });
    fireEvent.click(screen.getByRole("button", { name: "Книги, 12 записей" }));
    expect(onSelect).toHaveBeenCalledWith("books");
    fireEvent.click(screen.getByRole("button", { name: "Развернуть боковую панель" }));
    expect(onToggleCollapsed).toHaveBeenCalledTimes(1);
  });

  it("имя при наведении и фокусе: подсказка справа от полосы, уходит вместе с мышью и фокусом", () => {
    rail();
    const books = screen.getByRole("button", { name: "Книги, 12 записей" });
    expect(document.querySelector(".rail-tip")).toBeNull();
    fireEvent.pointerEnter(books);
    expect(document.querySelector(".rail-tip")).toHaveTextContent("Книги");
    expect(document.querySelector(".rail-tip")?.textContent).not.toContain("12");
    fireEvent.pointerLeave(books);
    expect(document.querySelector(".rail-tip")).toBeNull();
    fireEvent.focus(screen.getByRole("button", { name: /^Входящие/ }));
    expect(document.querySelector(".rail-tip")).toHaveTextContent("Входящие");
    fireEvent.blur(screen.getByRole("button", { name: /^Входящие/ }));
    expect(document.querySelector(".rail-tip")).toBeNull();
  });

  it("подсказка скрыта от диктора (имя уже в подписи кнопки)", () => {
    rail();
    fireEvent.pointerEnter(screen.getByRole("button", { name: "Избранное" }));
    expect(document.querySelector(".rail-tip")).toHaveAttribute("aria-hidden", "true");
  });

  it("контекстное меню работает и в полосе", () => {
    const { onChatAction } = rail();
    fireEvent.contextMenu(screen.getByRole("button", { name: "Книги, 12 записей" }));
    expect(itemNames()).toEqual(["Переименовать", "Сменить эмодзи", "Закрепить", "В архив"]);
    expect(document.querySelector(".rail-tip")).toBeNull();
    fireEvent.click(within(menu()).getByRole("menuitem", { name: "Сменить эмодзи" }));
    expect(onChatAction).toHaveBeenCalledWith("books", "icon");
  });

  it("корзина: точка и число в подписи, когда не пуста; aria-current при выбранной корзине", () => {
    const { unmount } = rail({ trashCount: 4, onOpenTrash: () => {}, selectedId: null });
    const trash = screen.getByRole("button", { name: "Корзина, 4 записи" });
    expect(trash).toHaveAttribute("aria-current", "true");
    expect(trash.querySelector(".rail-button__dot")).not.toBeNull();
    unmount();
    rail({ trashCount: 0, onOpenTrash: () => {} });
    expect(screen.getByRole("button", { name: "Корзина" }).querySelector(".rail-button__dot")).toBeNull();
  });

  it("корзина открывается щелчком; без onOpenTrash кнопки нет", () => {
    const onOpenTrash = vi.fn();
    const { unmount } = rail({ onOpenTrash });
    fireEvent.click(screen.getByRole("button", { name: "Корзина" }));
    expect(onOpenTrash).toHaveBeenCalledTimes(1);
    unmount();
    rail();
    expect(screen.queryByRole("button", { name: /Корзина/ })).toBeNull();
  });

  it("профиль: только аватар, имя в подписи; меню открывается и содержит «Настройки»", () => {
    const onOpenSettings = vi.fn();
    rail({ onOpenSettings });
    const avatar = screen.getByRole("button", { name: "Профиль: Я" });
    expect(avatar.querySelector(".profile__name")).toBeNull();
    fireEvent.click(avatar);
    fireEvent.click(within(screen.getByRole("menu", { name: "Профиль" })).getByRole("menuitem", { name: "Настройки" }));
    expect(onOpenSettings).toHaveBeenCalledTimes(1);
  });

  it("развёрнутый вид возвращает поиск, заголовки и имена", () => {
    const { rerender } = rail();
    rerender(<Sidebar chats={LIST} selectedId="books" onChatAction={() => {}} />);
    expect(screen.getByRole("searchbox")).toBeInTheDocument();
    expect(screen.getByText("Книги")).toBeInTheDocument();
    expect(document.querySelector("aside")).not.toHaveClass("sidebar--rail");
  });

  it("английский: подписи полосы", () => {
    render(<Sidebar chats={LIST} selectedId="books" collapsed onToggleCollapsed={() => {}} onOpenTrash={() => {}} trashCount={2} />, { language: "en" });
    expect(screen.getByRole("button", { name: "Expand sidebar" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Trash, 2 entries" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Profile: Me" })).toBeInTheDocument();
  });
});
