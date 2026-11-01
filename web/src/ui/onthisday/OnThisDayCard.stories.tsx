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

/** Один год: заголовок, крестик «Скрыть до завтра», «1 год назад · дата», запись со временем. */
export const ОдинГод: Story = { args: { card: ONE_YEAR } };

/** Три года подробно (до двух записей, «ещё N записей этого дня ›») и строки старых лет: «31 октября 2019 · 5 записей ›». */
export const ТриГодаИСтрокиСтарых: Story = {};

/** Длинная запись обрезается многоточием на третьей строке. */
export const ДлинныйТекст: Story = { args: { card: LONG_TEXT } };

/** Английский. */
export const Английский: Story = { globals: { language: "en" } };

/** Крупный текст. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** Узкая колонка (проверка переноса). */
export const Узкая: Story = {
  render: (args) => (
    <div style={{ maxWidth: "var(--layout-measure-column)" }}>
      <OnThisDayCard {...args} />
    </div>
  ),
};
