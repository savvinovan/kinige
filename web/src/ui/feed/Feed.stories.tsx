import type { Meta, StoryObj } from "@storybook/react-vite";
import { demoEntry, demoMessages } from "../../api/demoData";
import { Frame } from "../stories/Frame";
import { Feed } from "./Feed";

// «Сегодня» в историях фиксировано (вторник, 20 октября 2026), чтобы метки дней и снимки были стабильны.
const NOW = new Date(2026, 9, 20, 17, 0);
const TODAY = "2026-10-20";

const meta = {
  title: "Компоненты/Лента",
  component: Feed,
  args: { today: TODAY, messages: demoMessages(NOW) },
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
      demoEntry("a", TODAY, "09:12", "Первая мысль"),
      demoEntry("b", TODAY, "09:14", "Вторая, через две минуты"),
      demoEntry("c", TODAY, "09:17", "Третья, ещё через три"),
      demoEntry("d", TODAY, "09:22", "Четвёртая: пауза ровно пять минут, новая серия"),
    ],
  },
};

/** Метка прошлого года и текущего: «14 марта 2025» и «14 марта» (без года). */
export const МеткиДней: Story = {
  args: {
    messages: [
      demoEntry("a", "2025-03-14", "09:12", "Прошлый год"),
      demoEntry("b", "2026-03-14", "09:12", "Этот год"),
      demoEntry("c", "2026-10-19", "08:40", "Вчера"),
      demoEntry("d", TODAY, "09:12", "Сегодня"),
    ],
  },
};

/** Метки дней и подсказка по-английски: «March 14, 2025», «Yesterday», «Today». */
export const Английский: Story = { globals: { language: "en" } };
