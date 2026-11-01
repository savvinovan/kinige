import type { Meta, StoryObj } from "@storybook/react-vite";
import { demoEntry } from "../../api/demoData";
import { INBOX_ID } from "../../api/KinigeApi";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { MANY_YEARS } from "../onthisday/demoCard";
import { OnThisDayCard } from "../onthisday/OnThisDayCard";
import { ReplyStrip } from "../reply/ReplyStrip";
import type { SidebarChat } from "../sidebar/Sidebar";
import { MainWindowView } from "./MainWindowView";
import "./screens-stories.css";

// Слайс 5 в главном окне: карточка «В этот день» в начале сегодняшнего дня, ответ с цитатой, строка «Ответ на запись» над полем.
// «Сегодня» в историях фиксировано (суббота, 31 октября 2026).
const TODAY = "2026-10-31";
const noop = () => undefined;

const CHATS: SidebarChat[] = [
  { id: INBOX_ID, name: "", isInbox: true, emoji: "📥", count: 1724 },
  { id: "work", name: "Работа", emoji: "💼", count: 987, pinned: true },
  { id: "books", name: "Книги", emoji: "📚", count: 1234 },
  { id: "tg", name: "Анна", emoji: "💬", count: 12 },
];

const QUOTE: ReplyTo = {
  id: "old",
  chatId: INBOX_ID,
  journalDate: "2023-10-31",
  excerpt: "Дочитал «Конец вечности». Сильная концовка.",
  state: "live",
  yearsLater: 3,
};

const REPLY = { ...demoEntry("r1", "2026-10-30", "21:05", "До сих пор так думаю"), replyTo: QUOTE };

const MESSAGES = [
  demoEntry("y1", "2026-10-29", "18:40", "Вечером разобрал фотографии"),
  REPLY,
  demoEntry("t1", TODAY, "08:30", "Проснулся рано, до завтрака записал мысль"),
];

const card = <OnThisDayCard card={MANY_YEARS} onOpen={noop} onReply={noop} onDismiss={noop} />;

const meta = {
  title: "Экраны/Главное окно: в этот день и ответ",
  component: MainWindowView,
  parameters: { layout: "fullscreen" },
  args: {
    chats: CHATS,
    selectedChatId: INBOX_ID,
    messages: MESSAGES,
    today: TODAY,
    interactive: true,
    showSearch: false,
    composerButtons: { emoji: true },
    onThisDay: { journalDate: TODAY, node: card },
    onOpenReply: noop,
    sidebar: { collapsed: false, onToggleCollapsed: noop, onCreateChat: noop, onChatAction: noop, trashCount: 0, onOpenTrash: noop },
  },
} satisfies Meta<typeof MainWindowView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Inbox с карточкой под меткой «Сегодня», перед сегодняшней записью; вчерашний ответ с цитатой «3 года спустя». */
export const КарточкаВЛенте: Story = {};

/** Сегодня записей нет: карточка внизу ленты, над полем ввода. */
export const СегодняПусто: Story = { args: { messages: MESSAGES.slice(0, 2) } };

/** После «Ответить»: над полем строка «Ответ на запись · дата» и первая строка записи. */
export const ОтветНаЗапись: Story = {
  args: {
    draft: "До сих пор так думаю",
    replyStrip: <ReplyStrip quote={QUOTE} onCancel={noop} />,
  },
};

/** Английский, крупный текст. */
export const АнглийскийКрупный: Story = {
  args: {
    replyStrip: <ReplyStrip quote={QUOTE} onCancel={noop} />,
  },
  globals: { language: "en", textScale: "large" },
};
