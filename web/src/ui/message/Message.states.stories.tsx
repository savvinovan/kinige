import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import "../feed/feed.css";
import { Frame, FrameLabel } from "../stories/Frame";
import { Message } from "./Message";

// Слайс 3, продолжение `Message.actions.stories.tsx`: удалённая плашка, задача, избранное, теги, кликабельные квадраты.

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
/** Все обработчики есть: кнопки ряда и пункты меню живые. */
const live = { onEdit: noop, onToggleTask: noop, onToggleFavorite: noop, onChangeDate: noop, onDelete: noop };

/** Удалённая плашка: текст приглушён, вместо времени «Удалено · Вернуть»; ряда действий и меню нет. */
export const Удалённая: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} mode="deleted" onRestore={noop} />
    </div>
  ),
};

/** Удалённые: длинный текст (строка «Удалено · Вернуть» занимает место времени, последняя строка может перенестись), код в конце, задача, английский. */
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

/** Удалённая на английском, крупный текст. */
export const УдалённаяEnКрупный: Story = {
  globals: { language: "en", textScale: "large" },
  render: () => (
    <div className="story-column">
      <Message markdown="Buy bread and milk" sentAt={at(9, 12)} mode="deleted" onRestore={noop} />
    </div>
  ),
};

/** Фокус с клавиатуры на «Вернуть»: кольцо фокуса. */
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

/** Задача: перед текстом флажок. Не выполнена, выполнена (текст приглушён), длинный текст, избранная. */
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

/** Задача при наведении: ряд действий рядом, значок «задача» акцентный. */
export const ЗадачаНаведение: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Позвонить маме" sentAt={at(9, 12)} isTask showActions {...live} />
    </div>
  ),
};

/** Задача, английский, крупный текст. */
export const ЗадачаEnКрупный: Story = {
  globals: { language: "en", textScale: "large" },
  render: () => (
    <div className="story-column">
      <Message markdown="Call mom about the weekend" sentAt={at(9, 12)} isTask taskDone onToggleDone={noop} />
    </div>
  ),
};

/** Фокус с клавиатуры на флажке задачи. */
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

/** Избранное: тихая звезда левее времени; высота плашки не меняется, длинный текст тоже. */
export const Избранное: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} favorite />
      <Message markdown={LONG} sentAt={at(9, 14)} favorite />
      <Message markdown={`Вот код:\n\n${CODE}`} sentAt={at(9, 20)} favorite />
    </div>
  ),
};

/** Теги: подсвечены только те, что определил Rust; регистр сохраняется; тег в ссылке и в коде не подсвечен. */
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

/** Тег с фокусом с клавиатуры: кольцо фокуса вокруг `#идея`. */
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

/** Живые квадраты «- [ ]» в тексте: щелчок и пробел переключают (в истории — правкой текста Markdown, как в приложении). */
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

/** Фокус с клавиатуры на втором квадрате в тексте. */
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

/** Всё вместе: задача в избранном с тегом и квадратами, длинная, на английском и с крупным текстом. */
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
