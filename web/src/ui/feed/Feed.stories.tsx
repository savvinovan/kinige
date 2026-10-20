import type { Meta, StoryObj } from "@storybook/react-vite";
import { demoMessages } from "../../api/demoData";
import { Frame } from "../stories/Frame";
import { Feed } from "./Feed";

// «Сегодня» в историях фиксировано, чтобы метки дней и снимки были стабильны.
const NOW = new Date(2026, 9, 20, 17, 0);
const at = (hour: number, minute: number, day = 20) => new Date(2026, 9, day, hour, minute);

const meta = {
  title: "Компоненты/Лента",
  component: Feed,
  args: { now: NOW, messages: demoMessages(NOW) },
  decorators: [
    (Story) => (
      <Frame bottom>
        <div className="story-feed">
          <Story />
        </div>
      </Frame>
    ),
  ],
} satisfies Meta<typeof Feed>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Обычная: три метки дня («14 марта 2025», «Вчера», «Сегодня»), время у каждого сообщения. */
export const Обычная: Story = {};

/** Пустая: подсказка вместо ленты. */
export const Пусто: Story = { args: { messages: [] } };

/** Серия: пауза меньше 5 минут — плашки ближе друг к другу (9:12, 9:14, 9:17), пауза 5 минут и больше — обычный промежуток (9:22). */
export const Серия: Story = {
  args: {
    messages: [
      { id: "a", sentAt: at(9, 12), markdown: "Первая мысль" },
      { id: "b", sentAt: at(9, 14), markdown: "Вторая, через две минуты" },
      { id: "c", sentAt: at(9, 17), markdown: "Третья, ещё через три" },
      { id: "d", sentAt: at(9, 22), markdown: "Четвёртая: пауза ровно пять минут, новая серия" },
    ],
  },
};

/** Метка прошлого года и текущего: «14 марта 2025» и «14 марта» (без года). */
export const МеткиДней: Story = {
  args: {
    messages: [
      { id: "a", sentAt: new Date(2025, 2, 14, 9, 12), markdown: "Прошлый год" },
      { id: "b", sentAt: new Date(2026, 2, 14, 9, 12), markdown: "Этот год" },
      { id: "c", sentAt: at(8, 40, 19), markdown: "Вчера" },
      { id: "d", sentAt: at(9, 12), markdown: "Сегодня" },
    ],
  },
};
