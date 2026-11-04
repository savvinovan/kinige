import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import type { TrashItem } from "../../api/generated/TrashItem";
import { Frame } from "../stories/Frame";
import { TrashView } from "./TrashView";
import "./trash-stories.css";

// "Today" in the stories is pinned (Tuesday, 27 October 2026): day labels and deadlines are stable.
const TODAY = "2026-10-27";

const entry = (id: string, body: string, patch: Partial<TrashItem> = {}, journalDate = "2026-10-26"): TrashItem => ({
  message: {
    id,
    chatId: "c1",
    bodyMd: body,
    journalDate,
    createdAt: Date.UTC(2026, 9, 26, 9, 12),
    createdTz: "UTC",
    isTask: false,
    taskDoneAt: null,
    favoriteAt: null,
    tags: [],
    readOnly: false,
    origin: "user",
    replyTo: null,
    attachments: [],
  },
  chatTitle: "Входящие",
  chatIcon: "📥",
  chatIsInbox: true,
  chatColor: "grey",
  deletedAt: Date.UTC(2026, 9, 26, 18, 0),
  daysLeft: 29,
  ...patch,
});

const LONG =
  "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации, и записать вопросы, которые остались после чтения. Потом вернуться к этой записи через неделю и проверить, на сколько удалось ответить. Ещё раз пройтись по конспекту и выписать то, что осталось непонятным.";
const CODE = "```\nfn main() {\n    println!(\"Привет, Kinige\");\n}\n```";

const items: TrashItem[] = [
  entry("a", "Купить хлеб", {}, TODAY),
  entry("b", "### Планы на неделю\nЗакончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда.", {
    chatTitle: "Книги",
    chatIcon: "📚",
    chatIsInbox: false,
    daysLeft: 12,
  }),
  entry("c", "- [x] Поход в горы\n- [ ] Разобрать фотографии", { chatTitle: "Работа", chatIcon: "💼", chatIsInbox: false, daysLeft: 1 }, "2026-03-14"),
  entry("d", `Починил историю командой \`git rebase\`:\n\n${CODE}`, { chatTitle: "Код", chatIcon: "💻", chatIsInbox: false, daysLeft: 0 }, "2025-03-14"),
];

const meta = {
  title: "Компоненты/Корзина",
  component: TrashView,
  parameters: { layout: "fullscreen" },
  args: { items, language: "ru", today: TODAY, onRestore: () => undefined, onPurge: () => undefined },
  // The trash takes the window's full height (as in the app): a wrapper with the window height.
  render: (args) => (
    <div className="story-trash">
      <TrashView {...args} />
    </div>
  ),
  decorators: [
    (Story) => (
      <Frame bottom>
        <Story />
      </Frame>
    ),
  ],
} satisfies Meta<typeof TrashView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Regular: four entries: Inbox (named "Входящие"), a chat with an emoji, a task, code; the deadline "N days left" and "will be deleted today". */
export const Обычное: Story = {};

/** Empty trash: a quiet line. */
export const Пусто: Story = { args: { items: [] } };

/** Long text is clamped by lines (layout.trash.entry-lines) with an ellipsis. */
export const ДлинныйТекст: Story = { args: { items: [entry("a", LONG, { chatTitle: "Учёба", chatIcon: "🎓", chatIsInbox: false })] } };

/** Long chat names are cut with an ellipsis, the deadline on the right stays; Cyrillic and Latin. */
export const ДлинноеИмяЧата: Story = {
  args: {
    items: [
      entry("a", "Купить хлеб", {
        chatTitle: "Очень длинное имя чата про всё на свете and Some Long English Words After It",
        chatIcon: "🗂️",
        chatIsInbox: false,
      }),
    ],
  },
};

/** A chat without an emoji: the neutral 💬. A different year: "14 марта 2025". */
export const БезЭмодзиДругойГод: Story = {
  args: { items: [entry("a", "Запись без эмодзи у чата", { chatTitle: "Заметки", chatIcon: null, chatIsInbox: false }, "2025-03-14")] },
};

/** The error of the last action: a `role="alert"` line above the list. */
export const Ошибка: Story = { args: { error: "Запись не найдена: возможно, её удалили навсегда." } };

/** An entry being worked on (busyId): the bubble is muted, its buttons are unavailable. */
export const ЗаписьВРаботе: Story = { args: { busyId: "b" } };

/** English. */
export const Английский: Story = { args: { language: "en" }, globals: { language: "en" } };

/** Large text. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** The "Delete forever?" confirmation: the only confirmation in the app, focus on "Cancel". */
function WithConfirm(args: React.ComponentProps<typeof TrashView>) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector<HTMLButtonElement>(".trash__danger")?.click(), []);
  return (
    <div className="story-trash" ref={host}>
      <TrashView {...args} />
    </div>
  );
}
export const Подтверждение: Story = { render: (args) => <WithConfirm {...args} /> };

/** The confirmation in English, large text. */
export const ПодтверждениеEnКрупный: Story = {
  globals: { language: "en", textScale: "large" },
  args: { language: "en" },
  render: (args) => <WithConfirm {...args} />,
};

/** A live trash: "Restore" and a confirmed "Delete forever" remove the entry from the list. */
function Live(args: React.ComponentProps<typeof TrashView>) {
  const [list, setList] = useState(args.items);
  const remove = (id: string) => setList((current) => current.filter((i) => i.message.id !== id));
  return (
    <div className="story-trash">
      <TrashView {...args} items={list} onRestore={remove} onPurge={remove} />
    </div>
  );
}
export const Живая: Story = { render: (args) => <Live {...args} /> };

/** Keyboard focus on "Restore" of the first entry. */
function FocusedRestore(args: React.ComponentProps<typeof TrashView>) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector<HTMLButtonElement>(".trash-entry .button")?.focus(), []);
  return (
    <div className="story-trash" ref={host}>
      <TrashView {...args} />
    </div>
  );
}
export const Фокус: Story = { render: (args) => <FocusedRestore {...args} /> };
