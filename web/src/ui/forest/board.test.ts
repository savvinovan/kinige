import { describe, expect, it } from "vitest";
import { buildBoard, cellAt, lastSelectable, monthStarts, neighbourDay, rowDates, seasonCells } from "./board";
import { demoYear } from "./demoYear";
import { FOREST_2026, FOREST_2028_SUNDAY } from "./fixtures";

describe("the board built from a Rust year table", () => {
  const board = buildBoard(FOREST_2026);

  it("has a cell for every day, in the cell Rust gave it, and 7 x 53 cells", () => {
    expect(board.dims).toEqual({ rows: 7, cols: 53 });
    expect(board.cells).toHaveLength(7 * 53);
    expect(board.byDate.size).toBe(365);
    for (const day of FOREST_2026.days) {
      const cell = cellAt(board, day);
      expect(cell?.day?.date).toBe(day.date);
    }
    // 1 January 2026 is a Thursday: three empty cells above it in the first column.
    expect(cellAt(board, { row: 0, col: 0 })?.day).toBeNull();
    expect(cellAt(board, { row: 3, col: 0 })?.day?.date).toBe("2026-01-01");
  });

  it("paints back to front: the cells higher on the screen first", () => {
    const ys = board.order.map((i) => board.cells[i]?.center.y ?? 0);
    expect(ys).toEqual([...ys].sort((a, b) => a - b));
    expect(board.order).toHaveLength(board.cells.length);
  });

  it("only days up to today can be selected", () => {
    expect(board.byDate.get("2026-10-21")?.interactive).toBe(true);
    expect(board.byDate.get("2026-10-22")?.interactive).toBe(false);
    expect(board.byDate.get("2026-12-31")?.interactive).toBe(false);
    expect(lastSelectable(board)).toBe("2026-10-21");
    expect(lastSelectable(buildBoard(demoYear(2020, { today: "2019-05-05" })))).toBeNull();
    expect(lastSelectable(buildBoard(demoYear(2020, { today: "2030-05-05" })))).toBe("2020-12-31");
  });

  it("walks along the calendar by days and weeks, stopping at the ends of the year and at today", () => {
    expect(neighbourDay(board, "2026-03-01", 1)).toBe("2026-03-02");
    expect(neighbourDay(board, "2026-03-01", -1)).toBe("2026-02-28");
    expect(neighbourDay(board, "2026-03-01", 7)).toBe("2026-03-08");
    expect(neighbourDay(board, "2026-01-01", -1)).toBeNull();
    expect(neighbourDay(board, "2026-01-03", -7)).toBeNull();
    expect(neighbourDay(board, "2026-10-21", 1)).toBeNull();
    expect(neighbourDay(board, "2026-10-18", 7)).toBeNull();
    expect(neighbourDay(board, "2025-12-31", 1)).toBeNull();
    expect(neighbourDay(board, "not a date", 1)).toBeNull();
  });

  it("knows where months start and which date each weekday row shows", () => {
    const starts = monthStarts(board);
    expect(starts.map((s) => s.month)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    expect(starts[0]).toMatchObject({ month: 1, col: 0, date: "2026-01-01" });
    const dates = rowDates(board);
    expect(dates).toHaveLength(7);
    expect(dates.every((d) => d !== null)).toBe(true);
  });

  it("zooms to a season: winter is the January-February end, not the December one", () => {
    const winter = seasonCells(board, "winter");
    const dates = new Set(winter.map((c) => cellAt(board, c)?.day?.date.slice(5, 7)));
    expect([...dates].sort()).toEqual(["01", "02"]);
    expect(seasonCells(board, "autumn")).toHaveLength(30 + 31 + 30);
    expect(seasonCells(board, "summer")).toHaveLength(92);
    expect(seasonCells(board, "spring")).toHaveLength(92);
  });

  it("a 54-column year Sunday first keeps every day on the board", () => {
    const wide = buildBoard(FOREST_2028_SUNDAY);
    expect(wide.dims.cols).toBe(54);
    expect(wide.byDate.size).toBe(366);
    expect(cellAt(wide, { row: 0, col: 53 })?.day?.date).toBe("2028-12-31");
  });

  it("a damaged table does not break the board: a day outside the grid is dropped", () => {
    const damaged = { ...FOREST_2026, days: [...FOREST_2026.days, { ...FOREST_2026.days[0]!, date: "2026-13-40", row: 9, col: 999 }] };
    const rebuilt = buildBoard(damaged);
    expect(rebuilt.byDate.has("2026-13-40")).toBe(false);
    expect(rebuilt.byDate.size).toBe(365);
    expect(cellAt(rebuilt, { row: -1, col: 0 })).toBeNull();
    expect(cellAt(rebuilt, { row: 0, col: Number.NaN })).toBeNull();
  });
});
