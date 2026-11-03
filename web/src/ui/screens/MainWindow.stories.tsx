import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo } from "react";
import { DEMO_CHATS, DEMO_INBOX, demoEntry, demoMessages } from "../../api/demoData";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import type { Settings } from "../../api/generated/Settings";
import { DEFAULT_SHORTCUTS } from "../settings/demoShortcuts";
import { toSidebarChat } from "../sidebar/Sidebar";
import { MainWindow } from "./MainWindow";
import { MainWindowView } from "./MainWindowView";
import "./screens-stories.css";

// "Today" is fixed in the stories (Tuesday, 20 October 2026), so day labels and snapshots do not change from run to run.
const NOW = new Date(2026, 9, 20, 17, 0);
const TODAY = "2026-10-20";

const meta = {
  title: "Экраны/Главное окно",
  component: MainWindowView,
  parameters: { layout: "fullscreen" },
  args: { chats: DEMO_CHATS.map(toSidebarChat), selectedChatId: INBOX_ID, messages: demoMessages(NOW), today: TODAY },
} satisfies Meta<typeof MainWindowView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A 1280 x 800 window (the story window size in visual tests): sidebar, feed with day labels, composer. There is no chat title above the feed. */
export const Обычное: Story = {};

/** The same in English: day labels, time, captions, "Inbox". */
export const Английский: Story = { globals: { language: "en" } };

/** A narrow window: the feed narrows, plates wrap text, the sidebar stays. */
export const Узкое: Story = {
  decorators: [
    (Story) => (
      <div className="screen-narrow">
        <Story />
      </div>
    ),
  ],
};

/** An empty chat: a hint in the feed, the composer in place. */
export const ПустойЧат: Story = { args: { messages: [] } };

/** Long messages: plates hit the ~80 character width, a long string without spaces wraps. */
export const ДлинныйТекст: Story = {
  args: {
    messages: [
      demoEntry(
        "l1",
        TODAY,
        "09:12",
        "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации, и записать вопросы, которые остались после чтения. Потом вернуться к этой записи через неделю.",
      ),
      demoEntry(
        "l2",
        TODAY,
        "09:14",
        "Сверхъестественнонепроизносимоесловобезединогопробелаинизкогомногобуквенногообрыва и ссылка https://example.com/очень/длинный/путь/который/не/помещается/в/плашку",
      ),
      demoEntry("l3", TODAY, "11:30", "Mixed: русский текст and English words вперемешку, 0123456789 — «ёлочки»."),
    ],
  },
};

/** The real slice 1 window in the app: one entry, counter 1, an empty composer, no toast and no dead buttons (the entry simply appears). */
export const ПослеОтправки: Story = {
  args: {
    chats: [{ ...toSidebarChat(DEMO_INBOX), count: 1 }],
    messages: [demoEntry("n1", TODAY, "09:12", "Купить **молоко**\n- [ ] позвонить маме")],
    interactive: true,
    showSearch: false,
    composerButtons: {},
  },
};

/** The slice 1 app with an empty Inbox: an empty feed, the cursor in the composer, no counter on "Inbox". */
export const ПервыйЗапуск: Story = {
  args: {
    chats: [{ ...toSidebarChat(DEMO_INBOX), count: 0 }],
    messages: [],
    interactive: true,
    showSearch: false,
    composerButtons: {},
  },
};

/** A send error: the text stayed in the composer, the reason is above it. */
export const ОшибкаОтправки: Story = {
  args: {
    ...ПервыйЗапуск.args,
    draft: "Купить молоко",
    composerError: "Не удалось сохранить запись. Текст остался в поле.",
  },
};

/** The real `MainWindow` on `MockApi` (as in the app, but in memory): you can write and cancel. */
export const НаMockApi: Story = { render: () => <WindowOnMockApi /> };

function WindowOnMockApi() {
  const api = useMemo(() => new MockApi({ now: () => new Date(Date.UTC(2026, 9, 20, 14, 0)), messages: [] }), []);
  const settings: Settings = {
    ...DEFAULT_SETTINGS,
    mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
    shortcuts: DEFAULT_SHORTCUTS,
  };
  return <MainWindow api={api} initialSettings={settings} />;
}
