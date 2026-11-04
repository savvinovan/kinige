import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import { Sidebar, type ChatAction } from "./Sidebar";
import { type ChatWithoutTint, withTints } from "./storyChats";
import "./sidebar-stories.css";

const INBOX: ChatWithoutTint = { id: "inbox", name: "", isInbox: true, emoji: "📥", count: 42 };
const CHATS: ChatWithoutTint[] = [
  { id: "diary", name: "Дневник", emoji: "📔", count: 318, pinned: true },
  { id: "ideas", name: "Идеи", emoji: "💡", count: 87, pinned: true },
  INBOX,
  { id: "work", name: "Работа", emoji: "💼", count: 1234 },
  { id: "books", name: "Книги", emoji: "📚", count: 9 },
  { id: "recipes", name: "Рецепты", emoji: "🍳", count: 12345 },
  // Without an emoji: Rust stores "💬" by default, the sidebar shows a letter tile (the color comes from the id).
  { id: "projects", name: "Проекты", emoji: "💬", count: 18 },
  { id: "linux", name: "Linux notes", emoji: "💬", count: 55 },
  { id: "mom", name: "Мама", emoji: "💬", count: 214 },
  { id: "trip", name: "Отпуск 2025", emoji: "🏖️", count: 61, archived: true },
  { id: "old", name: "Старый проект", emoji: "💬", count: 4, archived: true },
];

interface DemoProps {
  chats?: readonly ChatWithoutTint[];
  selectedId?: string | null;
  collapsed?: boolean;
  trashCount?: number;
  /** Focuses the button of the selected chat: this is how the story shows the focus ring (and the rail tooltip) without a mouse. */
  focus?: boolean;
  /** Opens the context menu of the row whose name (or its beginning) is passed: like a right click. */
  menuFor?: string;
  /** Shows hover on the row whose name (or its beginning) is passed: `:hover` cannot be set in a story, so an attribute is set. */
  hoverFor?: string;
  showSearch?: boolean;
  /** The chat under the dragged entry (slice 3, moving an entry): highlighted as the target. */
  dropTargetId?: string | null;
}

/** The sidebar at full window height on an empty feed; selection, "collapse" and "+" work on the story state. */
function Demo({
  chats: initialChats = CHATS,
  selectedId: initialSelected = "work",
  collapsed: initialCollapsed = false,
  trashCount = 3,
  focus = false,
  menuFor,
  hoverFor,
  showSearch = false,
  dropTargetId = null,
}: DemoProps) {
  const [chats, setChats] = useState(() => withTints(initialChats));
  const [selectedId, setSelectedId] = useState<string | null>(initialSelected);
  const [collapsed, setCollapsed] = useState(initialCollapsed);
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const buttons = [...(host.current?.querySelectorAll<HTMLButtonElement>(".sidebar button") ?? [])];
    // A list row is labeled in title ("Книги, 9 записей"), a rail button in aria-label.
    const label = (button: HTMLButtonElement) => button.getAttribute("title") ?? button.getAttribute("aria-label") ?? "";
    if (focus) host.current?.querySelector<HTMLButtonElement>("[aria-current]")?.focus();
    // The "Корзина" row has neither `title` nor `aria-label`: the name is taken from the button text.
    if (hoverFor) {
      buttons.find((button) => (label(button) || (button.textContent ?? "")).startsWith(hoverFor))?.setAttribute("data-story-hover", "");
    }
    if (menuFor) {
      const row = buttons.find((button) => label(button).startsWith(menuFor));
      const rect = row?.getBoundingClientRect();
      if (row && rect) {
        row.dispatchEvent(
          new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2 }),
        );
      }
    }
  }, [focus, menuFor, hoverFor]);

  const act = (id: string, action: ChatAction) =>
    setChats((list) =>
      list.map((chat) => {
        if (chat.id !== id) return chat;
        if (action === "pin" || action === "unpin") return { ...chat, pinned: action === "pin" };
        if (action === "archive" || action === "unarchive") return { ...chat, archived: action === "archive", pinned: false };
        return chat;
      }),
    );

  return (
    <div className="story-sidebar" ref={host}>
      <Sidebar
        chats={chats}
        selectedId={selectedId}
        onSelect={setSelectedId}
        collapsed={collapsed}
        onToggleCollapsed={() => setCollapsed((was) => !was)}
        onCreateChat={() => {}}
        onChatAction={act}
        trashCount={trashCount}
        onOpenTrash={() => setSelectedId(null)}
        showSearch={showSearch}
        dropTargetId={dropTargetId}
        onOpenSettings={() => {}}
      />
    </div>
  );
}

const meta = {
  title: "Компоненты/Сайдбар",
  component: Demo,
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Default: "Закреплённые", "Чаты" with the "+" button, a collapsed "Архив"; at the bottom the "Корзина" with a counter and the profile. Everything is clickable. */
export const Обычное: Story = {};

/** With the "Поиск" field (it appears in slice 4): the "collapse" button stands next to it. */
export const СПоиском: Story = { args: { showSearch: true } };

/** A pinned chat is selected. */
export const ВыбранЗакреплённый: Story = { args: { selectedId: "diary" } };

/** There is only one chat, "Входящие": the "Чаты" header with "+" is in place, no other sections. */
export const ТолькоВходящие: Story = { args: { chats: [INBOX], selectedId: "inbox", trashCount: 0 } };

/** All chats are pinned: the "Чаты" section is empty, but the header with "+" stays: there must be a way to create a new chat. */
export const ВсёЗакреплено: Story = {
  args: { chats: [{ ...INBOX, pinned: true }, { id: "diary", name: "Дневник", emoji: "📔", count: 318, pinned: true }], selectedId: "inbox" },
};

/** A chat from the archive is selected: "Архив" is expanded by itself and cannot be collapsed while the chat is selected. */
export const АрхивРаскрыт: Story = { args: { selectedId: "trip" } };

/** The trash is selected: the "Корзина" row is highlighted, no chat is selected. */
export const КорзинаВыбрана: Story = { args: { selectedId: null } };

/** Long names are truncated with an ellipsis (the full name is in the tooltip) in every section, the emoji does not shrink. */
export const ДлинныеИмена: Story = {
  args: {
    chats: [
      { id: "a", name: "Подготовка к собеседованию по системному дизайну и распределённым системам", emoji: "🧭", count: 12345, pinned: true },
      { id: "b", name: "Сверхъестественнонепроизносимоесловобезединогопробела", emoji: "🧪", count: 1234567 },
      { id: "c", name: "Reading list: papers, blogs and long-form essays", emoji: "📖", count: 87 },
      { id: "d", name: "Архив поездок: Лиссабон, Порту, Мадейра and the Azores", emoji: "🧳", count: 9999, archived: true },
    ],
    selectedId: "a",
  },
};

/** Letter tiles of all six colors (the color is set explicitly); the green one is selected, the others on the background and next to the selected one. */
export const ЦветаПлиток: Story = {
  args: {
    chats: [
      { id: "b", name: "Синий", emoji: "💬", count: 4, tint: "blue" },
      { id: "t", name: "Бирюзовый", emoji: "💬", count: 5, tint: "teal" },
      { id: "g", name: "Зелёный", emoji: "💬", count: 6, tint: "green" },
      { id: "o", name: "Оранжевый", emoji: "💬", count: 7, tint: "orange" },
      { id: "p", name: "Розовый", emoji: "💬", count: 8, tint: "pink" },
      { id: "v", name: "Фиолетовый", emoji: "💬", count: 9, tint: "violet" },
    ],
    selectedId: "g",
    trashCount: 0,
  },
};

/** Emoji without a tile and letter tiles side by side: the names line up on one vertical; the letter is the first grapheme ("Ёлка", "Linux", "A" after an emoji). */
export const ЭмодзиИБуквы: Story = {
  args: {
    chats: [
      { id: "e1", name: "Идеи", emoji: "💡", count: 87 },
      { id: "l1", name: "Ёлка", emoji: "💬", count: 3 },
      { id: "e2", name: "Книги", emoji: "📚", count: 9 },
      { id: "l2", name: "Linux notes", emoji: "💬", count: 55 },
      { id: "l3", name: "  «мама» и папа", emoji: "💬", count: 214 },
      { id: "l4", name: "🎉 Party plans", emoji: "💬", count: 2 },
      { id: "l5", name: "!!!", emoji: "💬", count: 1 },
    ],
    selectedId: "l2",
    trashCount: 0,
  },
};

/** The same chats in the English interface (the names are user data). */
export const ЭмодзиИБуквыАнглийский: Story = {
  args: { ...ЭмодзиИБуквы.args },
  globals: { language: "en" },
};

/** Large text: the letter tile and the emoji grow with the text. */
export const ЭмодзиИБуквыКрупныйТекст: Story = {
  args: { ...ЭмодзиИБуквы.args },
  globals: { textScale: "large" },
};

/** The collapsed rail: emoji without a tile and letter tiles. */
export const ЭмодзиИБуквыСвёрнутый: Story = {
  args: { ...ЭмодзиИБуквы.args, collapsed: true },
};

/** The entry counter on the right: absent for an empty chat, 7, "1 234", "9 999", "12 тыс.", "1,2 млн"; quiet gray, not an accent. */
export const Счётчики: Story = {
  args: {
    chats: [
      { id: "a", name: "Пустой чат", emoji: "🫙", count: 0 },
      { id: "b", name: "Семь записей", emoji: "7️⃣", count: 7 },
      { id: "c", name: "Тысяча с лишним", emoji: "📗", count: 1234 },
      { id: "d", name: "Почти десять тысяч", emoji: "📘", count: 9999 },
      { id: "e", name: "Двенадцать тысяч", emoji: "📙", count: 12345 },
      { id: "f", name: "Миллион с лишним", emoji: "📕", count: 1234567 },
    ],
    selectedId: "c",
    trashCount: 12345,
  },
};

/** Many chats: the list scrolls, the "Корзина" and the profile stay at the bottom. */
export const МногоЧатов: Story = {
  args: { chats: Array.from({ length: 40 }, (_, i) => ({ id: `c${i}`, name: `Чат ${i + 1}`, emoji: "📌", count: (i + 1) * 3 })), selectedId: "c0" },
};

/** Hover: the "Книги" row is highlighted under the mouse (a quiet fill), the selected "Работа" has a denser fill and reads next to it. */
export const Наведение: Story = { args: { hoverFor: "Книги" } };

/** Hover on the "Корзина" at the bottom, and large text. */
export const НаведениеКрупныйТекст: Story = { args: { hoverFor: "Корзина" }, globals: { textScale: "large" } };

/** Keyboard focus on the row of the selected chat. */
export const Фокус: Story = { args: { focus: true } };

/** The menu of a regular chat: "Переименовать", "Сменить эмодзи", "Закрепить", "В архив". */
export const МенюЧата: Story = { args: { menuFor: "Книги" } };

/** The menu of a pinned chat: "Открепить" instead of "Закрепить". */
export const МенюЗакреплённого: Story = { args: { menuFor: "Идеи" } };

/** The "Входящие" menu: only "Закрепить"; it cannot be renamed or archived. */
export const МенюВходящих: Story = { args: { menuFor: "Входящие" } };

/** The menu of an archived chat: "Из архива", no pinning. */
export const МенюАрхивного: Story = { args: { selectedId: "trip", menuFor: "Отпуск" } };

/** Collapsed: the "emoji rail". "Expand" on top, at the bottom the "Корзина" with a dot and the avatar; no archived chats. */
export const Свёрнутый: Story = { args: { collapsed: true } };

/** Collapsed, keyboard focus on the selected chat: the focus ring and a tooltip with the name to the right of the rail. */
export const СвёрнутыйФокусИПодсказка: Story = { args: { collapsed: true, focus: true } };

/** Collapsed, the chat menu (right click on the emoji). */
export const СвёрнутыйМеню: Story = { args: { collapsed: true, menuFor: "Рецепты" } };

/** Collapsed, the trash is selected and empty: highlighted, no dot. */
export const СвёрнутыйКорзина: Story = { args: { collapsed: true, selectedId: null, trashCount: 0 } };

/** Collapsed, many chats: the rail scrolls, the "Корзина" and the avatar are at the bottom. */
export const СвёрнутыйМногоЧатов: Story = {
  args: {
    collapsed: true,
    chats: Array.from({ length: 40 }, (_, i) => ({ id: `c${i}`, name: `Чат ${i + 1}`, emoji: "📌", count: i + 1 })),
    selectedId: "c3",
  },
};

/** English interface: "Pinned", "Chats", "Archive", "Trash" (chat names are user data, not translated). */
/** An entry is dragged onto the "Книги" chat: the row is highlighted as the drop target (fill and a thin accent line), the current chat stays selected. */
export const ЦельПереноса: Story = { args: { dropTargetId: "books" } };

/** The drop target is a pinned chat. */
export const ЦельПереносаЗакреплённый: Story = { args: { dropTargetId: "ideas" } };

/** The drop target in the collapsed rail: the chat icon is highlighted. */
export const СвёрнутыйЦельПереноса: Story = { args: { collapsed: true, dropTargetId: "books" } };

/** The drop target is a chat from the expanded archive. */
export const ЦельПереносаАрхив: Story = { args: { selectedId: "trip", dropTargetId: "old" } };

/** The drop target with large text. */
export const ЦельПереносаКрупныйТекст: Story = { args: { dropTargetId: "books" }, globals: { textScale: "large" } };

export const Английский: Story = { args: { menuFor: "Книги" }, globals: { language: "en" } };

/** English, collapsed. */
export const СвёрнутыйАнглийский: Story = { args: { collapsed: true, focus: true }, globals: { language: "en" } };

/** Large text: rows, headers, counters and the rail grow with the text. */
export const КрупныйТекст: Story = { args: { selectedId: "trip" }, globals: { textScale: "large" } };

/** Large text, collapsed. */
export const СвёрнутыйКрупныйТекст: Story = { args: { collapsed: true, focus: true }, globals: { textScale: "large" } };
