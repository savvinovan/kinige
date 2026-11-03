import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState, type ReactNode } from "react";
import "../drag/drag.css";
import "../drag/drag-stories.css";
import "../feed/feed.css";
import { Frame } from "../stories/Frame";
import { Message } from "./Message";
import type { MoveTarget } from "./moveTargets";

// Slice 3: action row, context menu, editing in the bubble, deleted bubble, task, favorite, tags, clickable boxes,
// read-only entry. The first "Components/Message" stories (D1) are in Message.stories.tsx.

const at = (hour: number, minute: number) => new Date(2026, 9, 20, hour, minute);

const LONG =
  "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации, и записать вопросы, которые остались после чтения. Потом вернуться к этой записи через неделю.";
const MIXED = "Съешь же ещё этих мягких французских булок, да выпей чаю. The quick brown fox jumps over the lazy dog. #идея #ideas";

const meta = {
  title: "Компоненты/Сообщение: действия и состояния",
  component: Message,
  args: { markdown: "Купить хлеб", sentAt: at(9, 12) },
  decorators: [(Story) => <Frame><Story /></Frame>],
} satisfies Meta<typeof Message>;

export default meta;
type Story = StoryObj<typeof meta>;

const noop = () => undefined;
/** All handlers present: the row buttons and menu items are live. */
const live = { onEdit: noop, onToggleTask: noop, onToggleFavorite: noop, onChangeDate: noop, onDelete: noop };

/** Opens the bubble context menu after display: this is how the story shows an open menu without a mouse. */
function OpenMenu({ children, keyboard = false }: { children: ReactNode; keyboard?: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const plate = host.current?.querySelector("article");
    if (!plate) return;
    if (keyboard) {
      // From the keyboard: Shift+F10 on the "More" button opens the menu below it.
      const more = host.current?.querySelector<HTMLButtonElement>('button[aria-haspopup="menu"]');
      more?.focus();
      more?.dispatchEvent(new KeyboardEvent("keydown", { key: "F10", shiftKey: true, bubbles: true, cancelable: true }));
      return;
    }
    const rect = plate.getBoundingClientRect();
    plate.dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: rect.left + rect.width / 2, clientY: rect.bottom }),
    );
  }, [keyboard]);
  return (
    <div className="story-column" ref={host}>
      {children}
    </div>
  );
}

/** Action row on hover: task and favorite are off; all five buttons are live. */
export const РядДействий: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} showActions {...live} />
    </div>
  ),
};

/** Task and favorite are on: the "task" icon is accent, the star is filled, the screen reader labels are "Remove task", "Remove from favorites". */
export const РядДействийВключено: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} showActions isTask favorite {...live} />
    </div>
  ),
};

/** Without handlers (D1): "Edit", "Task", "Favorite" are drawn and disabled; "Copy" and "More" work. */
export const РядДействийБезОбработчиков: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} showActions />
    </div>
  ),
};

/** Menu on right click: "Copy", "Edit", "Make a task", "Add to favorites", "Change date...", "Delete" (dangerous, below a divider). The action row stays visible. */
export const МенюОткрыто: Story = {
  render: () => (
    <OpenMenu>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} {...live} />
    </OpenMenu>
  ),
};

/** Menu of a task entry in favorites: the items "Remove task" and "Remove from favorites". */
export const МенюЗадачаВИзбранном: Story = {
  render: () => (
    <OpenMenu>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} isTask favorite {...live} />
    </OpenMenu>
  ),
};

/** Menu opened from the keyboard (Shift+F10 on the "More" button): the first item is focused, the menu is under the button. */
export const МенюСКлавиатуры: Story = {
  render: () => (
    <OpenMenu keyboard>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} {...live} />
    </OpenMenu>
  ),
};

/** Menu in English. */
export const МенюEn: Story = {
  globals: { language: "en" },
  render: () => (
    <OpenMenu>
      <Message markdown="Buy bread" sentAt={at(9, 12)} {...live} />
    </OpenMenu>
  ),
};

/** Menu with large text. */
export const МенюКрупныйТекст: Story = {
  globals: { textScale: "large" },
  render: () => (
    <OpenMenu>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} {...live} />
    </OpenMenu>
  ),
};

const MOVE_TARGETS: MoveTarget[] = [
  { id: "ideas", name: "Идеи", emoji: "💡", group: "pinned", current: false },
  { id: "inbox", name: "Входящие", emoji: "📥", group: "chats", current: true },
  { id: "books", name: "Книги", emoji: "📚", group: "chats", current: false },
  { id: "recipes", name: "Рецепты на каждый день недели, которые не помещаются в меню", emoji: "🍳", group: "chats", current: false },
  { id: "trip", name: "Отпуск 2025", emoji: "🏖️", group: "archive", current: false },
];
const MOVE_TARGETS_EN: MoveTarget[] = [
  { id: "ideas", name: "Ideas", emoji: "💡", group: "pinned", current: false },
  { id: "inbox", name: "Inbox", emoji: "📥", group: "chats", current: true },
  { id: "books", name: "Books", emoji: "📚", group: "chats", current: false },
  { id: "trip", name: "Trip 2025", emoji: "🏖️", group: "archive", current: false },
];
const movable = { ...live, onMoveTo: noop };

/** Opens the entry menu and goes to the second level: "Move to chat" (the sixth item). */
function OpenMoveMenu({ children }: { children: ReactNode }) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const plate = host.current?.querySelector("article");
    if (!plate) return;
    const rect = plate.getBoundingClientRect();
    plate.dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: rect.left + rect.width / 2, clientY: rect.bottom }),
    );
    // The menu appears after rendering: the second level opens on the next frame.
    const frame = requestAnimationFrame(() => document.querySelectorAll<HTMLElement>('[role="menuitem"]')[5]?.click());
    return () => cancelAnimationFrame(frame);
  }, []);
  return (
    <div className="story-column" ref={host}>
      {children}
    </div>
  );
}

/** "Move to chat": the second menu level. "Back", pinned, regular and archived chats separated by thin dividers; the current chat is muted, a long name is cut with an ellipsis. */
export const МенюПеремещения: Story = {
  render: () => (
    <OpenMoveMenu>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} moveTargets={MOVE_TARGETS} {...movable} />
    </OpenMoveMenu>
  ),
};

/** The same in English. */
export const МенюПеремещенияEn: Story = {
  globals: { language: "en" },
  render: () => (
    <OpenMoveMenu>
      <Message markdown="Buy bread" sentAt={at(9, 12)} moveTargets={MOVE_TARGETS_EN} {...movable} />
    </OpenMoveMenu>
  ),
};

/** The same with large text. */
export const МенюПеремещенияКрупныйТекст: Story = {
  globals: { textScale: "large" },
  render: () => (
    <OpenMoveMenu>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} moveTargets={MOVE_TARGETS} {...movable} />
    </OpenMoveMenu>
  ),
};

/** The "ghost" of the bubble under the cursor while dragging to a chat: the same bubble, slightly more transparent, with a shadow; the cursor itself is not drawn. */
export const ПризракПеретаскивания: Story = {
  render: () => (
    <div className="story-column story-ghost">
      <div className="record-ghost" aria-hidden="true">
        <Message markdown="Купить хлеб" sentAt={at(9, 12)} />
      </div>
    </div>
  ),
};

/** Read-only entry (group newer than the supported version): buttons are muted with an explanation in the tooltip, only "Copy" is available in the menu. */
export const ТолькоЧтение: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Запись из новой версии Kinige" sentAt={at(9, 12)} showActions readOnly {...live} />
    </div>
  ),
};

/** Menu of a read-only entry. */
export const МенюТолькоЧтение: Story = {
  render: () => (
    <OpenMenu>
      <Message markdown="Запись из новой версии Kinige" sentAt={at(9, 12)} readOnly {...live} />
    </OpenMenu>
  ),
};

/** Editing, short text: a field instead of the text, the bubble width is the same, the time is in place; no action row. The focus line is inside the bubble. */
export const ПравкаКороткая: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} mode="editing" onSaveEdit={noop} onCancelEdit={noop} />
    </div>
  ),
};

/** Editing, long multi-line text with Cyrillic, Latin and a tag: the bubble grows only in height, up to the column width. */
export const ПравкаДлинная: Story = {
  render: () => (
    <div className="story-column">
      <Message
        markdown={`${LONG}\n\n${MIXED}`}
        sentAt={at(9, 12)}
        mode="editing"
        onSaveEdit={noop}
        onCancelEdit={noop}
        tags={["идея", "ideas"]}
      />
    </div>
  ),
};

/** Save-edit error: a `role="alert"` line inside the bubble, the text stays in the field. */
export const ПравкаОшибка: Story = {
  render: () => (
    <div className="story-column">
      <Message
        markdown="Купить хлеб и молоко"
        sentAt={at(9, 12)}
        mode="editing"
        onSaveEdit={noop}
        onCancelEdit={noop}
        editError="Не удалось сохранить запись. Текст остался в поле."
      />
    </div>
  ),
};

/** Editing in English, large text. */
export const ПравкаEnКрупный: Story = {
  globals: { language: "en", textScale: "large" },
  render: () => (
    <div className="story-column">
      <Message
        markdown="Buy bread and milk, then call Ivan about the weekend trip."
        sentAt={at(9, 12)}
        mode="editing"
        onSaveEdit={noop}
        onCancelEdit={noop}
        editError="Could not save the entry. The text stayed in the field."
      />
    </div>
  ),
};

/** Live editing: "Edit" in the row turns the bubble into a field (the width is pinned), Enter saves, Esc cancels. */
function EditDemo() {
  const [mode, setMode] = useState<"view" | "editing">("view");
  const [text, setText] = useState("Купить хлеб");
  return (
    <div className="story-column">
      <Message
        markdown={text}
        sentAt={at(9, 12)}
        mode={mode}
        showActions
        onEdit={() => setMode("editing")}
        onSaveEdit={(next) => {
          setText(next);
          setMode("view");
        }}
        onCancelEdit={() => setMode("view")}
      />
    </div>
  );
}
export const ПравкаЖивая: Story = { render: () => <EditDemo /> };

