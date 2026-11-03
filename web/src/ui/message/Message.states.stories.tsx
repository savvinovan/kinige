import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import "../feed/feed.css";
import { Frame, FrameLabel } from "../stories/Frame";
import { Message } from "./Message";

// Slice 3, continuation of `Message.actions.stories.tsx`: deleted bubble, task, favorite, tags, clickable boxes.

const at = (hour: number, minute: number) => new Date(2026, 9, 20, hour, minute);

const LONG =
  "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации, и записать вопросы, которые остались после чтения. Потом вернуться к этой записи через неделю.";
const MIXED = "Съешь же ещё этих мягких французских булок, да выпей чаю. The quick brown fox jumps over the lazy dog. #идея #ideas";
const CODE = "```\nfn main() {\n    println!(\"Привет, Kinige\");\n}\n```";

const meta = {
  title: "Компоненты/Сообщение: задача, избранное, теги, удаление",
  component: Message,
  args: { markdown: "Купить хлеб", sentAt: at(9, 12) },
  decorators: [(Story) => <Frame><Story /></Frame>],
} satisfies Meta<typeof Message>;

export default meta;
type Story = StoryObj<typeof meta>;

const noop = () => undefined;
/** All handlers present: the row buttons and menu items are live. */
const live = { onEdit: noop, onToggleTask: noop, onToggleFavorite: noop, onChangeDate: noop, onDelete: noop };

/** Deleted bubble: the text is muted, "Deleted · Restore" instead of the time; no action row and no menu. */
export const Удалённая: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} mode="deleted" onRestore={noop} />
    </div>
  ),
};

/** Deleted: long text (the "Deleted · Restore" line takes the place of the time, the last line may wrap), code at the end, task, English. */
export const УдалённыеРазные: Story = {
  render: () => (
    <div className="story-column">
      <FrameLabel>Длинный текст</FrameLabel>
      <Message markdown={LONG} sentAt={at(9, 12)} mode="deleted" onRestore={noop} />
      <FrameLabel>Кончается блоком кода: строка под блоком</FrameLabel>
      <Message markdown={`Вот код:\n\n${CODE}`} sentAt={at(9, 13)} mode="deleted" onRestore={noop} />
      <FrameLabel>Задача в избранном</FrameLabel>
      <Message markdown="Позвонить маме" sentAt={at(9, 14)} mode="deleted" isTask favorite onRestore={noop} />
    </div>
  ),
};

/** Deleted in English, large text. */
export const УдалённаяEnКрупный: Story = {
  globals: { language: "en", textScale: "large" },
  render: () => (
    <div className="story-column">
      <Message markdown="Buy bread and milk" sentAt={at(9, 12)} mode="deleted" onRestore={noop} />
    </div>
  ),
};

/** Keyboard focus on "Restore": the focus ring. */
function FocusRestore() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector<HTMLButtonElement>(".message__deleted button")?.focus(), []);
  return (
    <div className="story-column" ref={host}>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} mode="deleted" onRestore={noop} />
    </div>
  );
}
export const УдалённаяФокус: Story = { render: () => <FocusRestore /> };

/** Task: a checkbox before the text. Not done, done (the text is muted), long text, favorite. */
export const Задача: Story = {
  render: () => (
    <div className="story-column">
      <FrameLabel>Не выполнена</FrameLabel>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} isTask onToggleDone={noop} />
      <FrameLabel>Выполнена</FrameLabel>
      <Message markdown="Позвонить маме" sentAt={at(9, 14)} isTask taskDone onToggleDone={noop} />
      <FrameLabel>Длинный текст, в избранном</FrameLabel>
      <Message markdown={LONG} sentAt={at(10, 0)} isTask favorite onToggleDone={noop} />
      <FrameLabel>Только чтение: флажок не нажимается</FrameLabel>
      <Message markdown="Запись из новой версии" sentAt={at(10, 5)} isTask readOnly onToggleDone={noop} />
    </div>
  ),
};

/** Task on hover: the action row is next to it, the "task" icon is accent. */
export const ЗадачаНаведение: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Позвонить маме" sentAt={at(9, 12)} isTask showActions {...live} />
    </div>
  ),
};

/** Task, English, large text. */
export const ЗадачаEnКрупный: Story = {
  globals: { language: "en", textScale: "large" },
  render: () => (
    <div className="story-column">
      <Message markdown="Call mom about the weekend" sentAt={at(9, 12)} isTask taskDone onToggleDone={noop} />
    </div>
  ),
};

/** Keyboard focus on the task checkbox. */
function FocusCheck() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector<HTMLButtonElement>(".message__check")?.focus(), []);
  return (
    <div className="story-column" ref={host}>
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} isTask onToggleDone={noop} />
    </div>
  );
}
export const ЗадачаФокус: Story = { render: () => <FocusCheck /> };

/** Favorite: a quiet star to the left of the time; the bubble height does not change, long text too. */
export const Избранное: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} favorite />
      <Message markdown={LONG} sentAt={at(9, 14)} favorite />
      <Message markdown={`Вот код:\n\n${CODE}`} sentAt={at(9, 20)} favorite />
    </div>
  ),
};

/** Agent entry (slice 4a): a quiet "added by agent ·" mark in the time label style to the left of it; the bubble does not change, long text and favorite too. */
export const ЗаписьАгента: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Итоги встречи: договорились про Raft" sentAt={at(9, 12)} byAgent />
      <Message markdown={LONG} sentAt={at(9, 14)} byAgent />
      <Message markdown="Записано агентом и добавлено в избранное" sentAt={at(9, 16)} byAgent favorite />
      <Message markdown="Задача от агента" sentAt={at(9, 18)} byAgent isTask />
      <Message markdown={`Вот код:\n\n${CODE}`} sentAt={at(9, 20)} byAgent />
    </div>
  ),
};

/** Tags: only those determined by Rust are highlighted; case is preserved; a tag in a link and in code is not highlighted. */
export const Теги: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Мысль про #Идея и #работа, а #другое не тег" sentAt={at(9, 12)} tags={["идея", "работа"]} onTagClick={noop} />
      <Message markdown={MIXED} sentAt={at(9, 14)} tags={["идея", "ideas"]} onTagClick={noop} />
      <Message
        markdown={"`#идея` в коде, [#идея](https://example.com) в ссылке, a#идея не тег\n\n#идея в начале строки"}
        sentAt={at(9, 20)}
        tags={["идея"]}
        onTagClick={noop}
      />
    </div>
  ),
};

/** A tag with keyboard focus: the focus ring around `#идея`. */
function FocusTag() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector<HTMLElement>(".tag")?.focus(), []);
  return (
    <div className="story-column" ref={host}>
      <Message markdown="Мысль про #идея" sentAt={at(9, 12)} tags={["идея"]} onTagClick={noop} />
    </div>
  );
}
export const ТегФокус: Story = { render: () => <FocusTag /> };

/** Live "- [ ]" boxes in the text: click and Space toggle them (in the story, by editing the Markdown text, as in the app). */
function ItemsDemo() {
  const [text, setText] = useState("### Планы\n- [x] Поход в горы\n- [ ] Разобрать фотографии\n- [ ] Почитать книгу");
  const toggle = (index: number, checked: boolean) => {
    let n = -1;
    setText((current) =>
      current.replace(/\[[ xX]\]/g, (box) => {
        n += 1;
        return n === index ? (checked ? "[x]" : "[ ]") : box;
      }),
    );
  };
  return (
    <div className="story-column">
      <Message markdown={text} sentAt={at(11, 30)} onToggleTaskItem={toggle} />
    </div>
  );
}
export const КликабельныеКвадраты: Story = { render: () => <ItemsDemo /> };

/** Keyboard focus on the second box in the text. */
function FocusItem() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelectorAll<HTMLElement>(".task__box")[1]?.focus(), []);
  return (
    <div className="story-column" ref={host}>
      <Message markdown={"- [x] Поход в горы\n- [ ] Разобрать фотографии"} sentAt={at(11, 30)} onToggleTaskItem={noop} />
    </div>
  );
}
export const КвадратФокус: Story = { render: () => <FocusItem /> };

/** Everything together: a task in favorites with a tag and boxes, long, in English and with large text. */
export const ВсёВместеEnКрупный: Story = {
  globals: { language: "en", textScale: "large" },
  render: () => (
    <div className="story-column">
      <Message
        markdown={"Plan for the trip #travel\n- [x] Book tickets\n- [ ] Pack the bag"}
        sentAt={at(9, 12)}
        isTask
        favorite
        tags={["travel"]}
        onToggleTaskItem={noop}
        onTagClick={noop}
        onToggleDone={noop}
        showActions
        {...live}
      />
    </div>
  ),
};
