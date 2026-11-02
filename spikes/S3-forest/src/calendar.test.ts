import { describe, expect, it } from "vitest";
import {
  type FirstWeekday,
  type GridKind,
  allDays,
  dateAtCell,
  dayPosition,
  dayWeight,
  daysInYear,
  gridDims,
  isLeap,
  isoKey,
  seasonOfMonth,
  stageHistogram,
  stageOf,
  syntheticYear,
  weekdayMon0,
} from "./calendar";

const FIRSTS: FirstWeekday[] = ["mon", "sun"];
const GRIDS: GridKind[] = ["A", "B"];

// Golden table: (grid, first weekday, y-m-d) -> (row, col). The same rows are asserted by the Rust candidate in
// crates/kinige-domain/src/forest.rs; keep them identical.
const GOLDEN: [GridKind, FirstWeekday, number, number, number, number | null, number | null][] = [
  ["A", "mon", 2026, 1, 1, 0, 0],
  ["A", "sun", 2026, 2, 28, 1, 27],
  ["A", "mon", 2026, 2, 29, null, null],
  ["A", "mon", 2024, 2, 29, 1, 28],
  ["A", "mon", 2026, 4, 31, null, null],
  ["A", "mon", 2026, 12, 31, 11, 30],
  ["B", "mon", 2026, 1, 1, 3, 0],
  ["B", "sun", 2026, 1, 1, 4, 0],
  ["B", "mon", 2026, 12, 31, 3, 52],
  ["B", "sun", 2026, 12, 31, 4, 52],
  ["B", "mon", 2024, 1, 1, 0, 0],
  ["B", "mon", 2024, 2, 29, 3, 8],
  ["B", "mon", 2024, 12, 31, 1, 52],
  ["B", "sun", 2024, 1, 1, 1, 0],
  ["B", "sun", 2024, 12, 31, 2, 52],
  ["B", "mon", 2012, 1, 1, 6, 0],
  ["B", "mon", 2012, 12, 31, 0, 53],
  ["B", "sun", 2028, 1, 1, 6, 0],
  ["B", "sun", 2028, 12, 31, 0, 53],
  ["B", "mon", 2026, 2, 30, null, null],
];

describe("dayPosition golden table", () => {
  for (const [grid, first, y, m, d, row, col] of GOLDEN) {
    it(`${grid} ${first} ${y}-${m}-${d}`, () => {
      const pos = dayPosition(grid, { y, m, d }, first);
      expect(pos ? [pos.row, pos.col] : [null, null]).toEqual([row, col]);
    });
  }
});

describe("grid layout", () => {
  it("1 January 2026 is a Thursday (anchor for the golden table)", () => {
    expect(weekdayMon0(2026, 1, 1)).toBe(3);
  });

  it("A: 12 x 31, 7 missing dates in a common year and 6 in a leap year", () => {
    for (const [year, missing] of [[2026, 7], [2024, 6], [2100, 7], [2000, 6]] as const) {
      const dims = gridDims("A", year, "mon");
      expect(dims).toEqual({ rows: 12, cols: 31 });
      let empty = 0;
      for (let r = 0; r < 12; r++) for (let c = 0; c < 31; c++) if (!dateAtCell("A", year, "mon", { row: r, col: c })) empty++;
      expect(empty).toBe(missing);
    }
  });

  it("B needs a 54th column in exactly the leap years that start on the last day of the week", () => {
    const wide: Record<FirstWeekday, number[]> = { mon: [], sun: [] };
    for (const first of FIRSTS) {
      for (let y = 1990; y <= 2100; y++) {
        const dims = gridDims("B", y, first);
        expect(dims.rows).toBe(7);
        expect([53, 54]).toContain(dims.cols);
        if (dims.cols === 54) wide[first].push(y);
      }
    }
    expect(wide.mon).toEqual([2012, 2040, 2068, 2096]);
    expect(wide.sun).toEqual([2000, 2028, 2056, 2084]);
    for (const y of wide.mon) expect(isLeap(y)).toBe(true);
  });

  it("every day of every year 1990..2100 has a unique cell inside the grid and the inverse returns the same date", () => {
    for (const grid of GRIDS) {
      for (const first of FIRSTS) {
        for (let y = 1990; y <= 2100; y++) {
          const dims = gridDims(grid, y, first);
          const seen = new Set<number>();
          let n = 0;
          for (const date of allDays(y)) {
            const pos = dayPosition(grid, date, first);
            expect(pos).not.toBeNull();
            if (!pos) continue;
            expect(pos.row).toBeGreaterThanOrEqual(0);
            expect(pos.row).toBeLessThan(dims.rows);
            expect(pos.col).toBeGreaterThanOrEqual(0);
            expect(pos.col).toBeLessThan(dims.cols);
            const id = pos.row * 100 + pos.col;
            expect(seen.has(id)).toBe(false);
            seen.add(id);
            expect(dateAtCell(grid, y, first, pos)).toEqual(date);
            n++;
          }
          expect(n).toBe(daysInYear(y));
        }
      }
    }
  });

  it("B: the row is the weekday counted from the first weekday, the column is the week", () => {
    for (const first of FIRSTS) {
      const firstIdx = first === "mon" ? 0 : 6;
      for (const date of allDays(2026)) {
        const pos = dayPosition("B", date, first);
        expect(pos?.row).toBe((weekdayMon0(date.y, date.m, date.d) - firstIdx + 7) % 7);
      }
    }
  });

  it("years are laid out without gaps: consecutive days are neighbours in B", () => {
    const days = allDays(2025);
    for (let i = 1; i < days.length; i++) {
      const a = dayPosition("B", days[i - 1]!, "mon")!;
      const b = dayPosition("B", days[i]!, "mon")!;
      expect(b.row === (a.row + 1) % 7).toBe(true);
      expect(b.col).toBe(b.row === 0 ? a.col + 1 : a.col);
    }
  });

  it("rejects non-existent dates and out-of-range cells", () => {
    expect(dayPosition("A", { y: 2026, m: 13, d: 1 }, "mon")).toBeNull();
    expect(dayPosition("A", { y: 2026, m: 0, d: 1 }, "mon")).toBeNull();
    expect(dayPosition("B", { y: 2026, m: 6, d: 0 }, "sun")).toBeNull();
    expect(dateAtCell("A", 2026, "mon", { row: 12, col: 0 })).toBeNull();
    expect(dateAtCell("B", 2026, "mon", { row: -1, col: 0 })).toBeNull();
    expect(dateAtCell("B", 2026, "mon", { row: 0, col: 0 })).toBeNull(); // before 1 January
    expect(dateAtCell("B", 2026, "mon", { row: 6, col: 52 })).toBeNull(); // after 31 December
  });
});

describe("seasons", () => {
  it("northern and southern", () => {
    expect([12, 1, 2].map((m) => seasonOfMonth(m))).toEqual(["winter", "winter", "winter"]);
    expect([3, 4, 5].map((m) => seasonOfMonth(m))).toEqual(["spring", "spring", "spring"]);
    expect([6, 7, 8].map((m) => seasonOfMonth(m))).toEqual(["summer", "summer", "summer"]);
    expect([9, 10, 11].map((m) => seasonOfMonth(m))).toEqual(["autumn", "autumn", "autumn"]);
    expect(seasonOfMonth(1, true)).toBe("summer");
    expect(seasonOfMonth(7, true)).toBe("winter");
  });
});

describe("growth (PRD 7.2)", () => {
  it("weight: short messages without attachments do not count, long ones are capped, a photo adds half", () => {
    expect(dayWeight([])).toBe(0);
    expect(dayWeight([{ len: 2, attachments: 0 }])).toBe(0);
    expect(dayWeight([{ len: 100, attachments: 0 }])).toBeCloseTo(0.5);
    expect(dayWeight([{ len: 5000, attachments: 0 }])).toBe(1);
    expect(dayWeight([{ len: 1, attachments: 1 }])).toBeCloseTo(0.505);
    expect(dayWeight([{ len: 200, attachments: 2 }])).toBe(2);
  });

  it("stage: 0 for an empty day, 6 at w_max and beyond, monotonic", () => {
    expect(stageOf(0)).toBe(0);
    expect(stageOf(30)).toBe(6);
    expect(stageOf(10_000)).toBe(6);
    expect(stageOf(1)).toBe(1);
    expect(stageOf(5)).toBe(3);
    let prev = 0;
    for (let w = 0; w <= 40; w += 0.25) {
      const s = stageOf(w);
      expect(s).toBeGreaterThanOrEqual(prev);
      prev = s;
    }
  });
});

describe("synthetic year", () => {
  const data = syntheticYear(2026);

  it("covers every day, deterministically, with empty days and all seven stages", () => {
    expect(data.size).toBe(365);
    expect(isoKey({ y: 2026, m: 12, d: 31 })).toBe("2026-12-31");
    expect([...syntheticYear(2026).values()].map((v) => v.stage)).toEqual([...data.values()].map((v) => v.stage));
    const hist = stageHistogram(data);
    expect(hist).toHaveLength(7);
    for (const n of hist) expect(n).toBeGreaterThan(0);
    const empty = [...data.values()].filter((v) => v.notes === 0).length;
    expect(empty).toBeGreaterThanOrEqual(10);
    expect(empty).toBeLessThanOrEqual(50);
  });

  it("has the stage mix of a real journal, not a flat distribution", () => {
    const hist = stageHistogram(data);
    const low = (hist[0] ?? 0) + (hist[1] ?? 0) + (hist[2] ?? 0);
    const high = (hist[5] ?? 0) + (hist[6] ?? 0);
    expect(low).toBeGreaterThan(high);
  });
});
