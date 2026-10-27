import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import { Sidebar, type ChatAction, type SidebarChat } from "./Sidebar";
import "./sidebar-stories.css";

const INBOX: SidebarChat = { id: "inbox", name: "", isInbox: true, emoji: "📥", count: 42 };
const CHATS: SidebarChat[] = [
  { id: "diary", name: "Дневник", emoji: "📔", count: 318, pinned: true },
  { id: "ideas", name: "Идеи", emoji: "💡", count: 87, pinned: true },
  INBOX,
  { id: "work", name: "Работа", emoji: "💼", count: 1234 },
  { id: "books", name: "Книги", emoji: "📚", count: 9 },
  { id: "recipes", name: "Рецепты", emoji: "🍳", count: 12345 },
  { id: "reading", name: "Reading list", emoji: "📖" },
  { id: "trip", name: "Отпуск 2025", emoji: "🏖️", count: 61, archived: true },
  { id: "old", name: "Старый проект", emoji: "🗄️", count: 4, archived: true },
];

interface DemoProps {
  chats?: readonly SidebarChat[];
  selectedId?: string | null;
  collapsed?: boolean;
  trashCount?: number;
  /** Ставит фокус на кнопку выбранного чата: так история показывает кольцо фокуса (и подсказку в полосе) без мыши. */
  focus?: boolean;
  /** Открывает контекстное меню строки, имя (или его начало) которой передано: как правый щелчок. */
  menuFor?: string;
  showSearch?: boolean;
}

/** Сайдбар во всю высоту окна на фоне пустой ленты; выбор, «свернуть» и «+» работают на состоянии истории. */
function Demo({
  chats: initialChats = CHATS,
  selectedId: initialSelected = "work",
  collapsed: initialCollapsed = false,
  trashCount = 3,
  focus = false,
  menuFor,
  showSearch = false,
}: DemoProps) {
  const [chats, setChats] = useState(initialChats);
  const [selectedId, setSelectedId] = useState<string | null>(initialSelected);
  const [collapsed, setCollapsed] = useState(initialCollapsed);
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const buttons = [...(host.current?.querySelectorAll<HTMLButtonElement>(".sidebar button") ?? [])];
    // Строка списка подписана в title («Книги, 9 записей»), кнопка полосы — в aria-label.
    const label = (button: HTMLButtonElement) => button.getAttribute("title") ?? button.getAttribute("aria-label") ?? "";
    if (focus) host.current?.querySelector<HTMLButtonElement>("[aria-current]")?.focus();
    if (menuFor) {
      const row = buttons.find((button) => label(button).startsWith(menuFor));
      const rect = row?.getBoundingClientRect();
      if (row && rect) {
        row.dispatchEvent(
          new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2 }),
        );
      }
    }
  }, [focus, menuFor]);

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

/** Обычное: «Закреплённые», «Чаты» с кнопкой «+», свёрнутый «Архив»; внизу «Корзина» со счётчиком и профиль. Всё кликабельно. */
export const Обычное: Story = {};

/** С полем «Поиск» (оно появится в слайсе 4): кнопка «свернуть» стоит рядом с ним. */
export const СПоиском: Story = { args: { showSearch: true } };

/** Выбран закреплённый чат. */
export const ВыбранЗакреплённый: Story = { args: { selectedId: "diary" } };

/** Чат только один — «Входящие»: заголовок «Чаты» с «+» на месте, других разделов нет. */
export const ТолькоВходящие: Story = { args: { chats: [INBOX], selectedId: "inbox", trashCount: 0 } };

/** Все чаты закреплены: раздел «Чаты» пуст, но заголовок с «+» остаётся — создать новый чат должно быть чем. */
export const ВсёЗакреплено: Story = {
  args: { chats: [{ ...INBOX, pinned: true }, { id: "diary", name: "Дневник", emoji: "📔", count: 318, pinned: true }], selectedId: "inbox" },
};

/** Выбран чат из архива: «Архив» раскрыт сам и свернуть его нельзя, пока чат выбран. */
export const АрхивРаскрыт: Story = { args: { selectedId: "trip" } };

/** Выбрана корзина: подсвечена строка «Корзина», ни один чат не выбран. */
export const КорзинаВыбрана: Story = { args: { selectedId: null } };

/** Длинные имена обрезаются многоточием (полное имя — во всплывающей подсказке) в каждом разделе, эмодзи не сжимается. */
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

/** Счётчик записей справа: нет у пустого чата, 7, «1 234», «9 999», «12 тыс.», «1,2 млн»; тихий серый, не акцент. */
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

/** Много чатов: список прокручивается, «Корзина» и профиль остаются внизу. */
export const МногоЧатов: Story = {
  args: { chats: Array.from({ length: 40 }, (_, i) => ({ id: `c${i}`, name: `Чат ${i + 1}`, emoji: "📌", count: (i + 1) * 3 })), selectedId: "c0" },
};

/** Фокус с клавиатуры на строке выбранного чата. */
export const Фокус: Story = { args: { focus: true } };

/** Меню обычного чата: «Переименовать», «Сменить эмодзи», «Закрепить», «В архив». */
export const МенюЧата: Story = { args: { menuFor: "Книги" } };

/** Меню закреплённого чата: вместо «Закрепить» — «Открепить». */
export const МенюЗакреплённого: Story = { args: { menuFor: "Идеи" } };

/** Меню «Входящих»: только «Закрепить» — переименовать и архивировать их нельзя. */
export const МенюВходящих: Story = { args: { menuFor: "Входящие" } };

/** Меню архивного чата: «Из архива», закрепления нет. */
export const МенюАрхивного: Story = { args: { selectedId: "trip", menuFor: "Отпуск" } };

/** Свёрнутый: «полоса эмодзи». Сверху «развернуть», снизу «Корзина» с точкой и аватар; архивных чатов нет. */
export const Свёрнутый: Story = { args: { collapsed: true } };

/** Свёрнутый, фокус с клавиатуры на выбранном чате: кольцо фокуса и подсказка с именем справа от полосы. */
export const СвёрнутыйФокусИПодсказка: Story = { args: { collapsed: true, focus: true } };

/** Свёрнутый, меню чата (правый щелчок на эмодзи). */
export const СвёрнутыйМеню: Story = { args: { collapsed: true, menuFor: "Рецепты" } };

/** Свёрнутый, корзина выбрана и пуста: подсвечена, точки нет. */
export const СвёрнутыйКорзина: Story = { args: { collapsed: true, selectedId: null, trashCount: 0 } };

/** Свёрнутый, много чатов: полоса прокручивается, «Корзина» и аватар внизу. */
export const СвёрнутыйМногоЧатов: Story = {
  args: {
    collapsed: true,
    chats: Array.from({ length: 40 }, (_, i) => ({ id: `c${i}`, name: `Чат ${i + 1}`, emoji: "📌", count: i + 1 })),
    selectedId: "c3",
  },
};

/** Английский интерфейс: «Pinned», «Chats», «Archive», «Trash» (названия чатов — данные пользователя, не переводятся). */
export const Английский: Story = { args: { menuFor: "Книги" }, globals: { language: "en" } };

/** Английский, свёрнутый. */
export const СвёрнутыйАнглийский: Story = { args: { collapsed: true, focus: true }, globals: { language: "en" } };

/** Крупный текст: строки, заголовки, счётчики и полоса растут вместе с текстом. */
export const КрупныйТекст: Story = { args: { selectedId: "trip" }, globals: { textScale: "large" } };

/** Крупный текст, свёрнутый. */
export const СвёрнутыйКрупныйТекст: Story = { args: { collapsed: true, focus: true }, globals: { textScale: "large" } };
