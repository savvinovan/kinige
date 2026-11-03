import type { Meta, StoryObj } from "@storybook/react-vite";
import { demoEntry, demoMessages } from "../../api/demoData";
import { Frame } from "../stories/Frame";
import { Feed } from "./Feed";

// "Today" in the stories is pinned (Tuesday, 20 October 2026) so that day labels and snapshots are stable.
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

/** Regular: three day labels ("14 марта 2025", "Вчера", "Сегодня"), a time on every message. */
export const Обычная: Story = {};

/** Empty: a hint instead of the feed. */
export const Пусто: Story = { args: { messages: [] } };

/** Series: a pause under 5 minutes puts bubbles closer to each other (9:12, 9:14, 9:17), a pause of 5 minutes or more gives the regular gap (9:22). */
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

/** Labels of a past year and the current one: "14 марта 2025" and "14 марта" (no year). */
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

/** Day labels and the hint in English: "March 14, 2025", "Yesterday", "Today". */
export const Английский: Story = { globals: { language: "en" } };
