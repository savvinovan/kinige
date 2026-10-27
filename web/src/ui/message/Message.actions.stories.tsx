import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState, type ReactNode } from "react";
import "../drag/drag.css";
import "../drag/drag-stories.css";
import "../feed/feed.css";
import { Frame } from "../stories/Frame";
import { Message } from "./Message";
import type { MoveTarget } from "./moveTargets";

// Слайс 3: ряд действий, контекстное меню, правка в плашке, удалённая плашка, задача, избранное, теги, кликабельные квадраты,
// запись только для чтения. Первые истории «Компоненты/Сообщение» (D1) — в Message.stories.tsx.

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
/** Все обработчики есть: кнопки ряда и пункты меню живые. */
const live = { onEdit: noop, onToggleTask: noop, onToggleFavorite: noop, onChangeDate: noop, onDelete: noop };

/** Открывает контекстное меню плашки после показа: так история показывает открытое меню без мыши. */
function OpenMenu({ children, keyboard = false }: { children: ReactNode; keyboard?: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const plate = host.current?.querySelector("article");
    if (!plate) return;
    if (keyboard) {
      // С клавиатуры: Shift+F10 на кнопке «Ещё» открывает меню под ней.
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

/** Ряд действий при наведении: задача и избранное выключены; все пять кнопок живые. */
export const РядДействий: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} showActions {...live} />
    </div>
  ),
};

/** Задача и избранное включены: значок «задача» акцентный, звезда залитая, подписи для диктора — «Убрать задачу», «Убрать из избранного». */
export const РядДействийВключено: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} showActions isTask favorite {...live} />
    </div>
  ),
};

/** Без обработчиков (D1): «Редактировать», «Задача», «Избранное» нарисованы и недоступны; «Копировать» и «Ещё» работают. */
export const РядДействийБезОбработчиков: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} showActions />
    </div>
  ),
};

/** Меню по правому щелчку: «Копировать», «Редактировать», «Сделать задачей», «В избранное», «Изменить дату…», «Удалить» (опасное, за чертой). Ряд действий остаётся виден. */
export const МенюОткрыто: Story = {
  render: () => (
    <OpenMenu>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} {...live} />
    </OpenMenu>
  ),
};

/** Меню у записи-задачи в избранном: пункты «Убрать задачу» и «Убрать из избранного». */
export const МенюЗадачаВИзбранном: Story = {
  render: () => (
    <OpenMenu>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} isTask favorite {...live} />
    </OpenMenu>
  ),
};

/** Меню открыто с клавиатуры (Shift+F10 на кнопке «Ещё»): первый пункт в фокусе, меню под кнопкой. */
export const МенюСКлавиатуры: Story = {
  render: () => (
    <OpenMenu keyboard>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} {...live} />
    </OpenMenu>
  ),
};

/** Меню на английском. */
export const МенюEn: Story = {
  globals: { language: "en" },
  render: () => (
    <OpenMenu>
      <Message markdown="Buy bread" sentAt={at(9, 12)} {...live} />
    </OpenMenu>
  ),
};

/** Меню при крупном тексте. */
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

/** Открывает меню записи и переходит на второй уровень: «Переместить в чат» (шестой пункт). */
function OpenMoveMenu({ children }: { children: ReactNode }) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const plate = host.current?.querySelector("article");
    if (!plate) return;
    const rect = plate.getBoundingClientRect();
    plate.dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: rect.left + rect.width / 2, clientY: rect.bottom }),
    );
    document.querySelectorAll<HTMLElement>('[role="menuitem"]')[5]?.click();
  }, []);
  return (
    <div className="story-column" ref={host}>
      {children}
    </div>
  );
}

/** «Переместить в чат»: второй уровень меню. «Назад», закреплённые, обычные и архивные чаты тонкими чертами; текущий чат приглушён, длинное имя обрезано многоточием. */
export const МенюПеремещения: Story = {
  render: () => (
    <OpenMoveMenu>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} moveTargets={MOVE_TARGETS} {...movable} />
    </OpenMoveMenu>
  ),
};

/** То же на английском. */
export const МенюПеремещенияEn: Story = {
  globals: { language: "en" },
  render: () => (
    <OpenMoveMenu>
      <Message markdown="Buy bread" sentAt={at(9, 12)} moveTargets={MOVE_TARGETS_EN} {...movable} />
    </OpenMoveMenu>
  ),
};

/** То же при крупном тексте. */
export const МенюПеремещенияКрупныйТекст: Story = {
  globals: { textScale: "large" },
  render: () => (
    <OpenMoveMenu>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} moveTargets={MOVE_TARGETS} {...movable} />
    </OpenMoveMenu>
  ),
};

/** «Призрак» плашки под курсором при перетаскивании на чат: та же плашка, чуть прозрачнее, с тенью; сам курсор не рисуется. */
export const ПризракПеретаскивания: Story = {
  render: () => (
    <div className="story-column story-ghost">
      <div className="record-ghost" aria-hidden="true">
        <Message markdown="Купить хлеб" sentAt={at(9, 12)} />
      </div>
    </div>
  ),
};

/** Запись только для чтения (группа новее поддерживаемой версии): кнопки приглушены с пояснением в подсказке, в меню доступно только «Копировать». */
export const ТолькоЧтение: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Запись из новой версии Kinige" sentAt={at(9, 12)} showActions readOnly {...live} />
    </div>
  ),
};

/** Меню записи только для чтения. */
export const МенюТолькоЧтение: Story = {
  render: () => (
    <OpenMenu>
      <Message markdown="Запись из новой версии Kinige" sentAt={at(9, 12)} readOnly {...live} />
    </OpenMenu>
  ),
};

/** Правка, короткий текст: поле вместо текста, ширина плашки та же, время на месте; ряда действий нет. Линия фокуса внутри плашки. */
export const ПравкаКороткая: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} mode="editing" onSaveEdit={noop} onCancelEdit={noop} />
    </div>
  ),
};

/** Правка, длинный многострочный текст с кириллицей, латиницей и тегом: плашка растёт только по высоте, до ширины колонки. */
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

/** Ошибка сохранения правки: строка `role="alert"` внутри плашки, текст в поле остаётся. */
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

/** Правка на английском, крупный текст. */
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

/** Живая правка: «Редактировать» в ряду превращает плашку в поле (ширина закрепляется), Enter сохраняет, Esc отменяет. */
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

