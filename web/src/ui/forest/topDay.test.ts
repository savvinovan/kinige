import { describe, expect, it } from "vitest";
import { rect } from "../../test/rect";
import { topDayOf } from "../feed/topDay";

/** A scroller at y = 100..500 whose items sit at the given tops, 50 px high each. */
function scroller(items: { date?: string; at: number }[]): HTMLElement {
  const root = document.createElement("div");
  root.getBoundingClientRect = () => rect(0, 100, 600, 400);
  for (const item of items) {
    const li = document.createElement("li");
    if (item.date) li.dataset.journalDate = item.date;
    li.getBoundingClientRect = () => rect(0, item.at, 600, 50);
    root.append(li);
  }
  return root;
}

describe("the day at the top of the feed", () => {
  it("is the first element whose bottom edge is below the top of the scroller", () => {
    const root = scroller([
      { date: "2026-10-10", at: -200 },
      { date: "2026-10-11", at: 30 }, // bottom 80: above the top edge (100)
      { date: "2026-10-12", at: 60 }, // bottom 110: partly visible, so this is the top day
      { date: "2026-10-13", at: 160 },
    ]);
    expect(topDayOf(root)).toBe("2026-10-12");
  });

  it("is the first day when the feed is scrolled to its top and the last when only the bottom shows", () => {
    expect(topDayOf(scroller([{ date: "2026-10-10", at: 100 }, { date: "2026-10-11", at: 150 }]))).toBe("2026-10-10");
    expect(topDayOf(scroller([{ date: "2026-10-10", at: -900 }, { date: "2026-10-11", at: 90 }]))).toBe("2026-10-11");
  });

  it("skips elements without a date (the on-this-day card) and is null for an empty or fully scrolled-past feed", () => {
    expect(topDayOf(scroller([{ at: 100 }, { date: "2026-10-11", at: 160 }]))).toBe("2026-10-11");
    expect(topDayOf(scroller([]))).toBeNull();
    expect(topDayOf(scroller([{ date: "2026-10-10", at: -500 }]))).toBeNull();
  });

  it("works on a long feed", () => {
    const items = Array.from({ length: 5000 }, (_, i) => ({ date: `d${i}`, at: i * 50 - 123_000 }));
    expect(topDayOf(scroller(items))).toBe("d2462");
  });
});
