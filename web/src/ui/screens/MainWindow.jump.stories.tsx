import type { Meta, StoryObj } from "@storybook/react-vite";
import { DEMO_CHATS, demoMessages } from "../../api/demoData";
import { INBOX_ID } from "../../api/KinigeApi";
import { toSidebarChat } from "../sidebar/Sidebar";
import { MainWindowView } from "./MainWindowView";
import "./screens-stories.css";

// KINIG-145: the "Jump to latest" button in the main window, over the feed, above the composer. The feed holds only a window around an
// old entry (hasNewer), which is what the button shows for in a story; scrolling away more than a screen is checked in the Feed tests.
const NOW = new Date(2026, 9, 20, 17, 0);
const TODAY = "2026-10-20";

const meta = {
  title: "Экраны/Главное окно: к последним записям",
  component: MainWindowView,
  parameters: { layout: "fullscreen" },
  args: {
    chats: DEMO_CHATS.map(toSidebarChat),
    selectedChatId: INBOX_ID,
    messages: demoMessages(NOW),
    today: TODAY,
    feedPaging: {
      hasOlder: true,
      hasNewer: true,
      onLoadOlder: () => undefined,
      onLoadNewer: () => undefined,
      onJumpToLatest: () => Promise.resolve(),
    },
  },
} satisfies Meta<typeof MainWindowView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The window: the button at the bottom right of the feed, over it, the composer under the feed untouched. */
export const Обычное: Story = {};

/** Large text: the button and the composer grow, the button stays in the feed and does not reach the composer. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** A narrow window: the button overlays the right edge of the column. */
export const Узкое: Story = {
  decorators: [
    (Story) => (
      <div className="screen-narrow">
        <Story />
      </div>
    ),
  ],
};

/** A narrow window with large text. */
export const УзкоеКрупныйТекст: Story = {
  globals: { textScale: "large" },
  decorators: Узкое.decorators,
};

/** The same in English. */
export const Английский: Story = { globals: { language: "en" } };
