import type { Meta, StoryObj } from "@storybook/react-vite";
import { demoEntry } from "../../api/demoData";
import { INBOX_ID } from "../../api/KinigeApi";
import { FOREST_2026 } from "../forest/fixtures";
import { ForestPane } from "../forest/ForestPane";
import { withTints } from "../sidebar/storyChats";
import { MainWindowView } from "./MainWindowView";
import "./screens-stories.css";

// The forest in the main window (slice 6): the strip above the feed, the expanded board with the feed below it, and the narrow
// window. "Today" is Wednesday 21 October 2026 (the fixture year).
const TODAY = "2026-10-21";
const noop = () => undefined;

const CHATS = withTints([
  { id: INBOX_ID, name: "", isInbox: true, emoji: "📥", count: 1724 },
  { id: "work", name: "Работа", emoji: "💼", count: 987, pinned: true },
  { id: "books", name: "Книги", emoji: "📚", count: 1234 },
  { id: "long", name: "Очень длинное название чата, которое не помещается в строку: ".repeat(4).slice(0, 200), emoji: "📝", count: 3 },
]);

const MESSAGES = [
  demoEntry("y1", "2026-10-15", "18:40", "Вечером разобрал фотографии"),
  demoEntry("y2", "2026-10-16", "09:05", "Проснулся рано, до завтрака записал мысль про журнал"),
  demoEntry("y3", "2026-10-20", "21:05", "Закончил письмо, которое откладывал"),
  demoEntry("t1", TODAY, "08:30", "Сегодня спокойный день, окно открыто"),
];

function pane(view: "strip" | "expanded") {
  return (
    <ForestPane
      view={view}
      mode="forest"
      table={FOREST_2026}
      year={2026}
      minYear={2018}
      maxYear={2026}
      selected="2026-10-16"
      onOpenDay={noop}
      onYearChange={noop}
      onView={noop}
      onMode={noop}
    />
  );
}

const meta = {
  title: "Экраны/Главное окно: лес",
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
    sidebar: { collapsed: false, onToggleCollapsed: noop, onCreateChat: noop, onChatAction: noop, trashCount: 0, onOpenTrash: noop },
    forestMenu: { hidden: false, onToggle: noop },
  },
} satisfies Meta<typeof MainWindowView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Window 1280 x 800: the strip above the feed. */
export const StripAboveTheFeed: Story = { args: { forest: pane("strip") } };

/** The strip expanded into the isometric board; the feed stays below it. */
export const ExpandedAboveTheFeed: Story = { args: { forest: pane("expanded") } };

/**
 * The forest hidden (KINIG-134): the switch stays at the top right, where it was, not pressed; the feed starts under the bar.
 * Compare with `StripAboveTheFeed`: the button is in the same place.
 */
export const Hidden: Story = { args: { forestMenu: { hidden: true, onToggle: noop } } };

/** Hidden, English, large text: the switch keeps its place and does not touch the feed. */
export const HiddenEnglishLargeText: Story = {
  args: { forestMenu: { hidden: true, onToggle: noop } },
  globals: { language: "en", textScale: "large" },
};

/** Hidden in the 640 px window. */
export const NarrowHidden: Story = {
  args: { forestMenu: { hidden: true, onToggle: noop } },
  decorators: [
    (Story) => (
      <div className="screen-narrow">
        <Story />
      </div>
    ),
  ],
};

/** KINIG-150: the bar names the open chat at its left end; the switch stays at the right end, where the Inbox story has it. */
export const TitleShortName: Story = { args: { selectedChatId: "books", forest: pane("strip") } };

/** A 200-character name: one line cut with an ellipsis, the switch does not move. Shown and hidden, in the 640 px window. */
export const TitleVeryLongName: Story = { args: { selectedChatId: "long", forest: pane("strip") } };
export const TitleVeryLongNameHidden: Story = {
  args: { selectedChatId: "long", forestMenu: { hidden: true, onToggle: noop } },
  decorators: [
    (Story) => (
      <div className="screen-narrow">
        <Story />
      </div>
    ),
  ],
};

/** English, large text, a long name, the forest hidden. */
export const TitleLargeTextEnglish: Story = {
  args: { selectedChatId: "long", forestMenu: { hidden: true, onToggle: noop } },
  globals: { language: "en", textScale: "large" },
};

/**
 * KINIG-76: a long chat list (the seed's 19 chats) in a short window: only the list scrolls, never the page. What made the page scroll was the
 * screen reader's count text of each row, which sat outside the list's clip; the Playwright check (`main-window-size.spec.ts`) reads the page's size.
 */
export const ManyChats: Story = {
  args: { chats: [...CHATS, ...withTints(Array.from({ length: 16 }, (_, i) => ({ id: `many${i}`, name: `Чат ${i + 1}`, emoji: "💬", count: 3 + i })))] },
};

/** English, large text. */
export const EnglishLargeText: Story = { args: { forest: pane("strip") }, globals: { language: "en", textScale: "large" } };

/** The window narrowed to 640 px. */
export const NarrowExpanded: Story = {
  args: { forest: pane("expanded") },
  decorators: [
    (Story) => (
      <div className="screen-narrow">
        <Story />
      </div>
    ),
  ],
};
