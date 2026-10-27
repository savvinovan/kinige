import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "../../test/render";
import { partitionChats, toSidebarChat, type SidebarChat } from "./chats";
import { Sidebar } from "./Sidebar";

const chat = (id: string, name: string, extra: Partial<SidebarChat> = {}): SidebarChat => ({ id, name, emoji: "📌", ...extra });

const INBOX = chat("inbox", "", { isInbox: true, emoji: "📥", count: 3 });
const LIST: SidebarChat[] = [
  chat("fav", "Избранное", { pinned: true }),
  INBOX,
  chat("books", "Книги", { count: 12 }),
  chat("work", "Работа"),
  chat("old", "Старый проект", { archived: true }),
  chat("older", "Ещё старее", { archived: true, pinned: true }),
];

describe("разделы", () => {
  it("архив главнее закрепления, Inbox в архив не попадает, порядок внутри раздела сохраняется", () => {
    const sections = partitionChats([
      chat("a", "A", { pinned: true }),
      chat("b", "B"),
      chat("c", "C", { archived: true, pinned: true }),
      chat("inbox", "", { isInbox: true, archived: true }),
      chat("d", "D", { pinned: true }),
    ]);
    expect(sections.pinned.map((item) => item.id)).toEqual(["a", "d"]);
    expect(sections.chats.map((item) => item.id)).toEqual(["b", "inbox"]);
    expect(sections.archived.map((item) => item.id)).toEqual(["c"]);
  });

  it("toSidebarChat: флаги из времён; чат в архиве не считается закреплённым; пустое время — не закреплён", () => {
    const base = { id: "x", title: "Икс", icon: null, isInbox: false, count: 1 };
    expect(toSidebarChat({ ...base, pinnedAt: 5, archivedAt: null })).toMatchObject({ pinned: true, archived: false, emoji: "💬" });
    expect(toSidebarChat({ ...base, pinnedAt: 5, archivedAt: 6 })).toMatchObject({ pinned: false, archived: true });
    expect(toSidebarChat({ ...base, pinnedAt: null, archivedAt: null })).toMatchObject({ pinned: false, archived: false });
  });

  it("три раздела с тихими заголовками; «Закреплённые» и «Чаты» развёрнуты, «Архив» свёрнут", () => {
    render(<Sidebar chats={LIST} selectedId="books" showSearch={false} />);
    const pinned = within(screen.getByRole("group", { name: "Закреплённые" })).getAllByRole("button");
    const chats = within(screen.getByRole("group", { name: "Чаты" })).getAllByRole("button");
    expect(pinned).toHaveLength(1);
    expect(pinned[0]).toHaveTextContent("Избранное");
    expect(chats).toHaveLength(3);
    expect(chats[0]).toHaveTextContent("Входящие");
    expect(chats[1]).toHaveTextContent("Книги");
    expect(chats[2]).toHaveTextContent("Работа");
    const archive = screen.getByRole("button", { name: "Архив" });
    expect(archive).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("button", { name: /Старый проект/ })).toBeNull();
  });

  it("заголовок раздела виден, только если в нём есть чаты", () => {
    render(<Sidebar chats={[INBOX, chat("a", "А")]} selectedId="a" showSearch={false} />);
    expect(screen.queryByText("Закреплённые")).toBeNull();
    expect(screen.queryByRole("button", { name: "Архив" })).toBeNull();
    expect(screen.getByText("Чаты")).toBeInTheDocument();
  });

  it("«Архив» раскрывается щелчком и сворачивается обратно; aria-expanded и aria-controls", () => {
    render(<Sidebar chats={LIST} selectedId="books" showSearch={false} />);
    const archive = screen.getByRole("button", { name: "Архив" });
    fireEvent.click(archive);
    expect(archive).toHaveAttribute("aria-expanded", "true");
    const listId = archive.getAttribute("aria-controls") ?? "";
    expect(document.getElementById(listId)).toContainElement(screen.getByRole("button", { name: /Старый проект/ }));
    expect(screen.getByRole("button", { name: /Ещё старее/ })).toBeInTheDocument();
    fireEvent.click(archive);
    expect(archive).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("button", { name: /Старый проект/ })).toBeNull();
  });

  it("если выбран чат из архива, «Архив» раскрыт и свернуть его нельзя", () => {
    render(<Sidebar chats={LIST} selectedId="old" showSearch={false} />);
    const archive = screen.getByRole("button", { name: "Архив" });
    expect(archive).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: /Старый проект/ })).toHaveAttribute("aria-current", "true");
    fireEvent.click(archive);
    expect(archive).toHaveAttribute("aria-expanded", "true");
  });

  it("«+» у заголовка «Чаты» есть, пока есть onCreateChat, даже если все чаты закреплены", () => {
    const onCreateChat = vi.fn();
    const { rerender } = render(<Sidebar chats={[chat("p", "Закреп", { pinned: true })]} selectedId="p" showSearch={false} onCreateChat={onCreateChat} />);
    expect(screen.getByText("Чаты")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Новый чат" }));
    expect(onCreateChat).toHaveBeenCalledTimes(1);
    rerender(<Sidebar chats={LIST} selectedId="books" showSearch={false} />);
    expect(screen.queryByRole("button", { name: "Новый чат" })).toBeNull();
  });

  it("английский: заголовки и подписи", () => {
    render(<Sidebar chats={LIST} selectedId="books" showSearch={false} onCreateChat={() => {}} />, { language: "en" });
    expect(screen.getByRole("group", { name: "Pinned" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Chats" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Archive" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New chat" })).toBeInTheDocument();
  });

  it("длинные имена в разделах остаются в одной строке и доступны целиком в подсказке", () => {
    const long = "Подготовка к собеседованию по системному дизайну and distributed systems";
    render(<Sidebar chats={[chat("l", long, { pinned: true, count: 5 })]} selectedId="l" showSearch={false} />);
    expect(screen.getByRole("button", { name: new RegExp(long) })).toHaveAttribute("title", `${long}, 5 записей`);
  });
});

describe("верхняя строка и «Корзина»", () => {
  it("кнопка «свернуть» стоит в верхней строке рядом с поиском и вызывает onToggleCollapsed", () => {
    const onToggle = vi.fn();
    render(<Sidebar chats={LIST} selectedId="books" onToggleCollapsed={onToggle} />);
    const top = document.querySelector(".sidebar__top") as HTMLElement;
    expect(within(top).getByRole("searchbox", { name: "Поиск" })).toBeInTheDocument();
    fireEvent.click(within(top).getByRole("button", { name: "Свернуть боковую панель" }));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it("без поиска кнопка «свернуть» стоит одна; без обработчика её нет", () => {
    const { rerender } = render(<Sidebar chats={LIST} selectedId="books" showSearch={false} onToggleCollapsed={() => {}} />);
    expect(screen.getByRole("button", { name: "Свернуть боковую панель" })).toBeInTheDocument();
    rerender(<Sidebar chats={LIST} selectedId="books" showSearch={false} />);
    expect(screen.queryByRole("button", { name: "Свернуть боковую панель" })).toBeNull();
    expect(document.querySelector(".sidebar__top")).toBeNull();
  });

  it("строка «Корзина» над профилем: значок, имя, тихий счётчик; открывает корзину", () => {
    const onOpenTrash = vi.fn();
    render(<Sidebar chats={LIST} selectedId="books" showSearch={false} trashCount={7} onOpenTrash={onOpenTrash} />);
    const trash = screen.getByRole("button", { name: /^Корзина/ });
    expect(trash).toHaveTextContent("7");
    expect(trash).not.toHaveAttribute("aria-current");
    fireEvent.click(trash);
    expect(onOpenTrash).toHaveBeenCalledTimes(1);
    // Строка стоит после списка чатов и перед профилем.
    const nav = screen.getByRole("navigation", { name: "Чаты" });
    const profile = document.querySelector(".profile") as HTMLElement;
    expect(nav.compareDocumentPosition(trash) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(trash.compareDocumentPosition(profile) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(nav).not.toContainElement(trash);
  });

  it("выбрана корзина (selectedId = null): aria-current на «Корзине», ни один чат не отмечен", () => {
    render(<Sidebar chats={LIST} selectedId={null} showSearch={false} trashCount={0} onOpenTrash={() => {}} />);
    expect(screen.getByRole("button", { name: /^Корзина/ })).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("button", { name: /^Книги/ })).not.toHaveAttribute("aria-current");
    expect(document.querySelector(".chat-row--selected .chat-row__count")).toBeNull();
  });

  it("пустая корзина: счётчика нет; без onOpenTrash строки нет", () => {
    const { rerender } = render(<Sidebar chats={LIST} selectedId="books" showSearch={false} trashCount={0} onOpenTrash={() => {}} />);
    expect(screen.getByRole("button", { name: "Корзина" })).not.toHaveTextContent(/\d/);
    rerender(<Sidebar chats={LIST} selectedId="books" showSearch={false} />);
    expect(screen.queryByRole("button", { name: /Корзина/ })).toBeNull();
  });
});
