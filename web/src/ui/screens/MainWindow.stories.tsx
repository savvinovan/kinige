import type { Meta, StoryObj } from "@storybook/react-vite";
import { DEMO_CHATS, demoMessages } from "../../api/demoData";
import { MainWindowView } from "./MainWindowView";
import "./screens-stories.css";

// «Сегодня» в историях фиксировано (вторник, 20 октября 2026), чтобы метки дней и снимки не менялись от запуска к запуску.
const NOW = new Date(2026, 9, 20, 17, 0);

const meta = {
  title: "Экраны/Главное окно",
  component: MainWindowView,
  parameters: { layout: "fullscreen" },
  args: { chats: DEMO_CHATS, selectedChatId: "inbox", messages: demoMessages(NOW), now: NOW },
} satisfies Meta<typeof MainWindowView>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Окно 1280 × 800 (размер окна истории в визуальных тестах): сайдбар, лента с метками дней, поле ввода. Названия чата над лентой нет. */
export const Обычное: Story = {};

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

/** Временный выбор размера текста: 13 px (панель Storybook «Размер текста»). Плотность меняется целиком. */
export const Текст13: Story = { globals: { textSize: "13" } };

/** Временный выбор размера текста: 15 px. */
export const Текст15: Story = { globals: { textSize: "15" } };

/** Пустой чат: подсказка в ленте, поле ввода на месте. */
export const ПустойЧат: Story = { args: { messages: [] } };

/** Длинные сообщения: плашки упираются в ширину ~80 знаков, длинная строка без пробелов переносится. */
export const ДлинныйТекст: Story = {
  args: {
    messages: [
      {
        id: "l1",
        sentAt: new Date(2026, 9, 20, 9, 12),
        markdown:
          "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации, и записать вопросы, которые остались после чтения. Потом вернуться к этой записи через неделю.",
      },
      {
        id: "l2",
        sentAt: new Date(2026, 9, 20, 9, 14),
        markdown: "Сверхъестественнонепроизносимоесловобезединогопробелаинизкогомногобуквенногообрыва и ссылка https://example.com/очень/длинный/путь/который/не/помещается/в/плашку",
      },
      { id: "l3", sentAt: new Date(2026, 9, 20, 11, 30), markdown: "Mixed: русский текст and English words вперемешку, 0123456789 — «ёлочки»." },
    ],
  },
};
