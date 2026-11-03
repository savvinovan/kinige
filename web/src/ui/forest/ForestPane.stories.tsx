import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import type { ForestMode } from "../../api/generated/ForestMode";
import type { ForestYear } from "../../api/generated/ForestYear";
import { demoYear } from "./demoYear";
import { FOREST_2025, FOREST_2026, FOREST_2028_SUNDAY } from "./fixtures";
import type { ForestEngine } from "./engine";
import { ForestBoard, type ZoomRequest } from "./ForestBoard";
import { ForestPane } from "./ForestPane";
import "./forest-stories.css";

// The forest above the feed: the expanded pane with its header, the board at each level of detail, the selection with its card.
const noop = () => undefined;

const meta = {
  title: "Forest/Pane and board",
  parameters: { layout: "fullscreen" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

interface PaneStoryProps {
  table?: ForestYear;
  mode?: ForestMode;
  selected?: string | null;
  view?: "strip" | "expanded";
  narrow?: boolean;
}

/** The pane at the top of the main area, with the stand-in feed below it. */
function PaneStory({ table = FOREST_2026, mode = "forest", selected = "2026-09-12", view = "expanded", narrow = false }: PaneStoryProps) {
  return (
    <div className={narrow ? "forest-story forest-story--narrow" : "forest-story"}>
      <ForestPane
        view={view}
        mode={mode}
        table={table}
        year={table.year}
        minYear={2018}
        maxYear={2026}
        selected={selected}
        onOpenDay={noop}
        onYearChange={noop}
        onView={noop}
        onMode={noop}
        onHide={noop}
      />
      <div className="forest-story__feed">The feed</div>
    </div>
  );
}

/** The whole year of 2026 on the board: winter at both ends, spring, summer and autumn between; empty days are bare ground. */
export const WholeYear: Story = { render: () => <PaneStory selected={null} /> };

export const WholeYearEnglish: Story = { ...WholeYear, globals: { language: "en" } };

/** A selected day: outlined, the trees in front of it semi-transparent, its card in the corner. */
export const SelectedDayWithCard: Story = { render: () => <PaneStory selected="2026-10-16" /> };

/** 2028 starts on a Saturday, Sunday first: 54 columns. */
export const Year54Columns: Story = { render: () => <PaneStory table={FOREST_2028_SUNDAY} selected="2028-03-03" /> };

/** A past year, complete to 31 December. */
export const PastYear: Story = { render: () => <PaneStory table={FOREST_2025} selected="2025-12-30" /> };

/** A year without entries: bare ground and nothing else. */
export const EmptyYear: Story = { render: () => <PaneStory table={demoYear(2021, { today: "2026-10-21", minYear: 2018 })} selected={null} /> };

/** The calendar alternative under the same header. */
export const CalendarMode: Story = { render: () => <PaneStory mode="calendar" /> };

export const CalendarModeEnglish: Story = { ...CalendarMode, globals: { language: "en" } };

/** The window narrowed to 640 px: the header wraps, the board is drawn smaller, everything stays reachable. */
export const NarrowWindow: Story = { render: () => <PaneStory narrow selected="2026-09-12" /> };

/** The strip above the stand-in feed. */
export const StripAboveFeed: Story = { render: () => <PaneStory view="strip" /> };

function Box({ small = false, zoom = null, selected = null }: { small?: boolean; zoom?: ZoomRequest | null; selected?: string | null }) {
  return (
    <div className={small ? "forest-story__box forest-story__box--small" : "forest-story__box forest-story__box--wide"}>
      <ForestBoard table={FOREST_2026} selected={selected} zoom={zoom} onOpenDay={noop} />
    </div>
  );
}

/** Dots: a tile narrower than 14 px (a very small window). */
export const LevelDot: Story = { render: () => <Box small /> };

/** Simplified shapes: a tile narrower than 32 px (the whole year in the window). */
export const LevelSimplified: Story = { render: () => <Box /> };

/** Detailed shapes: a tile of 32 px and more (zoomed to autumn). */
export const LevelDetailed: Story = { render: () => <Box zoom={{ target: "autumn", nonce: 1 }} selected="2026-10-16" /> };

/** The "Winter" zoom shows January and February, not the December at the other end. */
export const WinterZoom: Story = { render: () => <Box zoom={{ target: "winter", nonce: 1 }} /> };

declare global {
  interface Window {
    /** Set by the click-through stories: where every day that can be opened sits on the canvas, in page pixels. */
    __forestProbe?: () => { date: string; x: number; y: number }[];
  }
}

/** The year with "today" at its last day, so that every day of it can be opened (the fixtures stop at a day in the middle of the year). */
function wholeYear(table: ForestYear): ForestYear {
  return { ...table, today: table.days[table.days.length - 1]?.date ?? table.today };
}

/**
 * The board with a probe for the visual click-through test (tests/visual/forest.spec.ts): the test asks where each day is and
 * clicks it with a real pointer; the day that the board opened is written next to the board.
 */
function ClickThrough({ table }: { table: ForestYear }) {
  const [opened, setOpened] = useState("");
  const [canvas, setCanvas] = useState<ForestEngine | null>(null);
  const probe = (engine: ForestEngine | null) => {
    setCanvas(engine);
    window.__forestProbe = engine
      ? () => {
          const box = document.querySelector("canvas")?.getBoundingClientRect();
          if (!box) return [];
          return table.days
            .filter((day) => day.date <= table.today)
            .flatMap((day) => {
              const at = engine.screenOf(day.date);
              return at ? [{ date: day.date, x: box.left + at.x, y: box.top + at.y }] : [];
            });
        }
      : undefined;
  };
  return (
    <div>
      <div className="forest-story__box forest-story__box--wide">
        <ForestBoard table={table} selected={null} zoom={null} onOpenDay={setOpened} onEngine={probe} />
      </div>
      <output data-testid="opened" data-ready={canvas ? "yes" : "no"}>
        {opened}
      </output>
    </div>
  );
}

/** Every day of 2026 (Monday first) can be clicked: the test clicks each one and the board opens that day. */
export const ClickThrough2026: Story = { render: () => <ClickThrough table={wholeYear(FOREST_2026)} /> };

/** The same for 2028 (Sunday first, 54 columns). */
export const ClickThrough2028Sunday: Story = { render: () => <ClickThrough table={wholeYear(FOREST_2028_SUNDAY)} /> };
