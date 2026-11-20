import type { Meta, StoryObj } from "@storybook/react-vite";
import { demoEntry } from "../../api/demoData";
import { INBOX_ID } from "../../api/KinigeApi";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { FOREST_2026 } from "../forest/fixtures";
import { ForestPane } from "../forest/ForestPane";
import { ReplyStrip } from "../reply/ReplyStrip";
import { withTints } from "../sidebar/storyChats";
import { MainWindowView } from "./MainWindowView";
import "./screens-stories.css";

// The smallest main window, 640 x 400 (KINIG-76), at normal and large text: the document never scrolls, only the feed does, and the top
// bar, the forest switch and the field stay whole. "Today" is Wednesday 21 October 2026 (the fixture year).
const TODAY = "2026-10-21";
const noop = () => undefined;

const CHATS = withTints([
  { id: INBOX_ID, name: "", isInbox: true, emoji: "📥", count: 1724 },
  { id: "work", name: "Работа", emoji: "💼", count: 987, pinned: true },
]);

const MESSAGES = [
  demoEntry("y1", "2026-10-15", "18:40", "Вечером разобрал фотографии"),
  demoEntry("y2", "2026-10-16", "09:05", "Проснулся рано, до завтрака записал мысль про журнал"),
  demoEntry("y3", "2026-10-20", "21:05", "Закончил письмо, которое откладывал"),
  demoEntry("t1", TODAY, "08:30", "Сегодня спокойный день, окно открыто"),
];

const QUOTE: ReplyTo = {
  id: "y2",
  chatId: INBOX_ID,
  journalDate: "2026-10-16",
  excerpt: "Проснулся рано, до завтрака записал мысль про журнал.",
  state: "live",
  yearsLater: 0,
  attachments: { photos: 0, files: 0, voices: 0, voiceMs: null, name: null },
};

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
  title: "Экраны/Главное окно: наименьшее окно",
  component: MainWindowView,
  parameters: { layout: "fullscreen" },
  decorators: [
    (Story) => (
      <div className="story-main-min">
        <Story />
      </div>
    ),
  ],
  args: {
    chats: CHATS,
    selectedChatId: INBOX_ID,
    messages: MESSAGES,
    today: TODAY,
    interactive: true,
    showSearch: false,
    composerButtons: { emoji: true },
    sidebar: { collapsed: false, onToggleCollapsed: noop, onCreateChat: noop, onChatAction: noop, trashCount: 0, onOpenTrash: noop },
    forestMenu: { hidden: true, onToggle: noop },
  },
} satisfies Meta<typeof MainWindowView>;

export default meta;
type Story = StoryObj<typeof meta>;

const LARGE = { language: "en", textScale: "large" } as const;
const SHOWN = { hidden: false, onToggle: noop };
const REPLY = <ReplyStrip quote={QUOTE} onCancel={noop} />;

/** The forest hidden: the bar, the feed, the field. */
export const Hidden: Story = {};
export const HiddenLargeText: Story = { globals: LARGE };

/** The strip above the feed. */
export const Strip: Story = { args: { forest: pane("strip"), forestMenu: SHOWN } };
export const StripLargeText: Story = { args: { forest: pane("strip"), forestMenu: SHOWN }, globals: LARGE };

/** The expanded board: it gives way to the feed, never pushes the field out of the window. */
export const Expanded: Story = { args: { forest: pane("expanded"), forestMenu: SHOWN } };
export const ExpandedLargeText: Story = { args: { forest: pane("expanded"), forestMenu: SHOWN }, globals: LARGE };

/** A reply strip above the field takes more of the window: the field is still whole. */
export const ReplyStripHidden: Story = { args: { replyStrip: REPLY } };
export const ReplyStripStripLargeText: Story = { args: { replyStrip: REPLY, forest: pane("strip"), forestMenu: SHOWN }, globals: LARGE };
export const ReplyStripExpandedLargeText: Story = { args: { replyStrip: REPLY, forest: pane("expanded"), forestMenu: SHOWN }, globals: LARGE };
