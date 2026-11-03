import type { Meta, StoryObj } from "@storybook/react-vite";
import { Frame } from "../stories/Frame";
import { LONG_TEXT, MANY_YEARS, ONE_YEAR } from "./demoCard";
import { OnThisDayCard } from "./OnThisDayCard";

const noop = () => undefined;

const meta = {
  title: "Компоненты/Карточка «В этот день»",
  component: OnThisDayCard,
  args: { card: MANY_YEARS, onOpen: noop, onReply: noop, onDismiss: noop },
  decorators: [
    (Story) => (
      <Frame>
        <div className="story-column">
          <Story />
        </div>
      </Frame>
    ),
  ],
} satisfies Meta<typeof OnThisDayCard>;

export default meta;
type Story = StoryObj<typeof meta>;

/** One year: the heading, the "Скрыть до завтра" (hide until tomorrow) cross, "1 год назад · date", an entry with its time. */
export const ОдинГод: Story = { args: { card: ONE_YEAR } };

/** Three years in detail (up to two entries, "ещё N записей этого дня >") and rows of older years: "31 октября 2019 · 5 записей >". */
export const ТриГодаИСтрокиСтарых: Story = {};

/** A long entry is cut with an ellipsis on the third line. */
export const ДлинныйТекст: Story = { args: { card: LONG_TEXT } };

/** English. */
export const Английский: Story = { globals: { language: "en" } };

/** Large text. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** A narrow column (a wrapping check). */
export const Узкая: Story = {
  render: (args) => (
    <div style={{ maxWidth: "var(--layout-measure-column)" }}>
      <OnThisDayCard {...args} />
    </div>
  ),
};
