import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { SearchHit } from "../../api/generated/SearchHit";
import { Frame } from "../stories/Frame";
import { NO_FILTERS, SearchResults } from "./SearchResults";
import "./search-stories.css";

// Search results in place of the feed. "Today" in the stories is pinned (Tuesday, 27 October 2026).
const TODAY = "2026-10-27";
const noop = () => undefined;
const TEAL = "teal";

const hit = (id: string, snippet: string, terms: string[], patch: Partial<SearchHit> = {}, journalDate = TODAY): SearchHit => ({
  message: {
    id,
    chatId: "c1",
    bodyMd: snippet,
    journalDate,
    createdAt: Date.UTC(2026, 9, 27, 9, 12),
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
  chatTitle: "",
  chatIcon: "📥",
  chatIsInbox: true,
  chatColor: "grey",
  snippet,
  terms,
  ...patch,
});

const HITS: SearchHit[] = [
  hit("a", "Читаю книгу про Raft и распределённые транзакции, пока в дороге", ["книгу"]),
  hit("b", "Книга о Rust: глава про владение и заимствование", ["Книга"], { chatTitle: "Книги", chatIcon: "📚", chatIsInbox: false }, "2026-10-26"),
  hit(
    "c",
    "…разобрать, что в книге сказано про консенсус, и записать вопросы; книгами из списка занимаюсь по вечерам, а пока откладываю…",
    ["книге"],
    { chatTitle: "Учёба", chatIcon: "🎓", chatIsInbox: false },
    "2025-03-14",
  ),
  hit("d", "Очень-очень длинная строка без пробелов для проверки переноса: КнигаКнигаКнигаКнигаКнигаКнигаКнигаКнигаКнигаКнига", ["Книга"], {
    chatTitle: "Очень длинное название чата, которое не помещается в строку and Some Long English Words",
    chatIcon: "🗂️",
    chatIsInbox: false,
  }),
];

const CHAT: ChatSummary = { id: "books", title: "Книги", icon: "📚", isInbox: false, count: 1234, pinnedAt: null, archivedAt: null, color: TEAL };

const meta = {
  title: "Экраны/Поиск",
  component: SearchResults,
  parameters: { layout: "fullscreen" },
  args: {
    listId: "results",
    query: "книги",
    chats: [],
    hits: HITS,
    hasMore: false,
    loading: false,
    searched: true,
    error: null,
    today: TODAY,
    language: "ru",
    filters: NO_FILTERS,
    onFiltersChange: noop,
    canScopeChat: true,
    onPickDate: noop,
    activeIndex: 0,
    onActiveChange: noop,
    onOpenChat: noop,
    onOpenHit: noop,
    onLoadMore: noop,
  },
  render: (args) => (
    <div className="story-search">
      <Frame bottom>
        <SearchResults {...args} />
      </Frame>
    </div>
  ),
} satisfies Meta<typeof SearchResults>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Entries with highlighted words: Inbox ("Входящие"), a chat with an emoji, another year, a long chat name and a word without spaces. */
export const Результаты: Story = {};

/** A chat whose name matched appears as a separate "Chats" row above the entries. */
export const СЧатами: Story = { args: { chats: [CHAT], activeIndex: 1 } };

/** KINIG-59: chats without an emoji have a letter tile in their colour in the chat row and in the entry rows (grey, pink, violet). */
export const ПлиткиЧатов: Story = {
  args: {
    chats: [{ ...CHAT, icon: "💬", color: "violet" }],
    hits: [
      hit("t1", "Проверка плитки серого чата", ["плитки"], { chatTitle: "Linux notes", chatIcon: "💬", chatIsInbox: false, chatColor: "grey" }),
      hit("t2", "Проверка плитки розового чата", ["плитки"], { chatTitle: "Проекты", chatIcon: "💬", chatIsInbox: false, chatColor: "pink" }),
      hit("t3", "Проверка плитки фиолетового чата", ["плитки"], { chatTitle: "Книги", chatIcon: "💬", chatIsInbox: false, chatColor: "violet" }, "2026-10-26"),
    ],
    activeIndex: 1,
  },
};

/** A row selected with the arrows: only a noticeable fill (`highlight.row`), no border. */
export const ВыбраннаяСтрока: Story = { args: { activeIndex: 2 } };

/** Filters: this chat, favorites, tasks, both dates. */
export const Фильтры: Story = {
  args: { filters: { scope: "chat", favorite: true, tasks: true, from: "2026-01-01", to: "2026-10-26" } },
};

/** There is a next page: "Show more". */
export const ЕщёСтраница: Story = { args: { hasMore: true } };

/** Searching: a request is in flight. */
export const Ищем: Story = { args: { hits: [], searched: false, loading: true } };

/** An empty result: a single quiet line. */
export const Пусто: Story = { args: { hits: [], query: "ничего" } };

/** An empty query: a hint. */
export const ПустойЗапрос: Story = { args: { hits: [], query: "", searched: false } };

/** A read error: the D1 error line. */
export const Ошибка: Story = { args: { hits: [], error: "store_read_failed" } };

/** English. */
export const Английский: Story = { args: { chats: [CHAT], language: "en" }, globals: { language: "en" } };

/** Large text. */
export const КрупныйТекст: Story = { args: { chats: [CHAT] }, globals: { textScale: "large" } };
