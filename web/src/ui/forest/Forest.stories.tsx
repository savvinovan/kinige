import type { Meta, StoryObj } from "@storybook/react-vite";
import { DayCard } from "./DayCard";
import { demoYear } from "./demoYear";
import { FOREST_2026, FOREST_2028_SUNDAY } from "./fixtures";
import { ForestCalendar } from "./ForestCalendar";
import { ForestStrip } from "./ForestStrip";
import "./forest-stories.css";

// The flat parts of the forest: the strip, the calendar and the day card. "Today" is fixed by the fixtures (Wednesday 21 October
// 2026; 15 June 2028 for the Sunday-first year), so the pictures do not change from run to run.
const noop = () => undefined;

const meta = {
  title: "Forest/Flat views",
  parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const frame = (child: React.ReactNode, narrow = false) => (
  <div className={narrow ? "forest-story forest-story--narrow" : "forest-story"}>
    <section className="forest-pane" aria-label="Forest">
      {child}
    </section>
  </div>
);

/** The collapsed forest: every day of the year as a cell tinted by its season, denser for fuller days; the day at the top of the feed is outlined. */
export const Strip: Story = {
  render: () => frame(<ForestStrip table={FOREST_2026} selected="2026-09-12" onOpenDay={noop} />),
};

export const StripEnglish: Story = { ...Strip, globals: { language: "en" } };

/** A leap year that starts on the last day of the week: 54 columns, Sunday first. */
export const Strip54Columns: Story = {
  render: () => frame(<ForestStrip table={FOREST_2028_SUNDAY} selected="2028-03-03" onOpenDay={noop} />),
};

/** A year without a single entry: bare ground, no text. */
export const StripEmptyYear: Story = {
  render: () => frame(<ForestStrip table={demoYear(2021, { today: "2026-10-21", minYear: 2021 })} selected={null} onOpenDay={noop} />),
};

/** A narrow window: the cells shrink so that all the weeks still fit. */
export const StripNarrow: Story = {
  render: () => frame(<ForestStrip table={FOREST_2026} selected="2026-09-12" onOpenDay={noop} />, true),
};

/** Twelve ordinary month grids with the same data: a dot sized by the stage under each number. */
export const Calendar: Story = {
  render: () => frame(<ForestCalendar table={FOREST_2026} selected="2026-09-12" onOpenDay={noop} />),
};

export const CalendarEnglish: Story = { ...Calendar, globals: { language: "en" } };

/** Sunday first: the first of each month sits under its own weekday. */
export const CalendarSundayFirst: Story = {
  render: () => frame(<ForestCalendar table={FOREST_2028_SUNDAY} selected="2028-03-03" onOpenDay={noop} />),
};

const longDay = FOREST_2026.days.find((d) => d.date === "2026-10-16")!;
const bigDay = FOREST_2026.days.find((d) => d.stage === 6)!;
const emptyDay = FOREST_2026.days.find((d) => d.date === "2026-08-05")!;
const bareDay = FOREST_2026.days.find((d) => d.notes > 0 && d.stage === 0)!;

/** The card: date, "N notes · stage" and the first line of the first entry, cut after two lines. Long excerpt, a large tree, a day without notes, a day with only short notes. */
export const DayCards: Story = {
  render: () => (
    <div className="forest-story__cards">
      <DayCard day={longDay} />
      <DayCard day={bigDay} />
      <DayCard day={emptyDay} />
      <DayCard day={bareDay} />
    </div>
  ),
};

export const DayCardsEnglish: Story = { ...DayCards, globals: { language: "en" } };
