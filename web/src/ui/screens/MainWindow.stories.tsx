import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo } from "react";
import { DEMO_CHATS, DEMO_INBOX, demoEntry, demoMessages } from "../../api/demoData";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import type { Settings } from "../../api/generated/Settings";
import { DEFAULT_SHORTCUTS } from "../settings/demoShortcuts";
import { SettingsDialog } from "../settings/SettingsDialog";
import { toSidebarChat } from "../sidebar/Sidebar";
import { MainWindow } from "./MainWindow";
import { MainWindowView } from "./MainWindowView";
import "./screens-stories.css";

// «Сегодня» в историях фиксировано (вторник, 20 октября 2026), чтобы метки дней и снимки не менялись от запуска к запуску.
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

/** Окно 1280 × 800 (размер окна истории в визуальных тестах): сайдбар, лента с метками дней, поле ввода. Названия чата над лентой нет. */
export const Обычное: Story = {};

/** То же по-английски: метки дней, время, подписи, «Inbox». */
export const Английский: Story = { globals: { language: "en" } };

/** Узкое окно: лента сужается, плашки переносят текст, сайдбар остаётся. */
export const Узкое: Story = {
  decorators: [
    (Story) => (
      <div className="screen-narrow">
        <Story />
      </div>
    ),
  ],
};

/** Пустой чат: подсказка в ленте, поле ввода на месте. */
export const ПустойЧат: Story = { args: { messages: [] } };

/** Длинные сообщения: плашки упираются в ширину ~80 знаков, длинная строка без пробелов переносится. */
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

/** Настоящее окно слайса 1 в приложении: одна запись, счётчик 1, пустое поле, без тоста и без мёртвых кнопок (запись просто появляется). */
export const ПослеОтправки: Story = {
  args: {
    chats: [{ ...toSidebarChat(DEMO_INBOX), count: 1 }],
    messages: [demoEntry("n1", TODAY, "09:12", "Купить **молоко**\n- [ ] позвонить маме")],
    interactive: true,
    showSearch: false,
    composerButtons: {},
  },
};

/** Приложение слайса 1 с пустым Inbox: пустая лента, курсор в поле, у «Входящих» нет счётчика. */
export const ПервыйЗапуск: Story = {
  args: {
    chats: [{ ...toSidebarChat(DEMO_INBOX), count: 0 }],
    messages: [],
    interactive: true,
    showSearch: false,
    composerButtons: {},
  },
};

/** Ошибка отправки: текст остался в поле, над ним — причина. */
export const ОшибкаОтправки: Story = {
  args: {
    ...ПервыйЗапуск.args,
    draft: "Купить молоко",
    composerError: "Не удалось сохранить запись. Текст остался в поле.",
  },
};

/** Окно настроек поверх главного окна (русский). */
export const НастройкиОткрыты: Story = {
  args: {
    ...ПослеОтправки.args,
    overlay: (
      <SettingsDialog
        settings={{
          ...DEFAULT_SETTINGS,
          mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
          shortcuts: DEFAULT_SHORTCUTS,
        }}
        shortcuts={DEFAULT_SHORTCUTS}
        onShortcutChange={() => undefined}
        onShortcutReset={() => undefined}
        onShortcutRecording={() => undefined}
        onClose={() => undefined}
        onChooseFolder={() => undefined}
        onTextSizeChange={() => undefined}
      />
    ),
  },
};

/** Настоящий `MainWindow` на `MockApi` (как в приложении, но в памяти): можно писать, отменять и открывать настройки. */
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
