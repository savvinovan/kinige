import type { ForestDay } from "../../api/generated/ForestDay";
import type { ForestYear } from "../../api/generated/ForestYear";
import type { Season } from "../../api/generated/Season";
import { type Cell, type GridDims, type Pt, cellCenter } from "./iso";

// The board as data: one entry per cell of the grid, in painter's order. The table (which date sits in which cell, its
// season, notes and stage) comes from Rust; this module only indexes it.

export interface BoardCell {
  row: number;
  col: number;
  /** World position of the cell centre. */
  center: Pt;
  /** `null`: a cell around the year (before 1 January, after 31 December): ground, no interaction. */
  day: ForestDay | null;
  /** The day exists, is not after today and so can be hovered, clicked and selected. */
  interactive: boolean;
}

export interface Board {
  year: number;
  dims: GridDims;
  /** Row-major: `cells[row * cols + col]`. */
  cells: BoardCell[];
  /** Indices into `cells`, back to front (rows + cols ascending). */
  order: number[];
  byDate: Map<string, BoardCell>;
  today: string;
  /** The days in calendar order (the keyboard walks along it). */
  days: readonly ForestDay[];
}

const ROWS = 7;

export function buildBoard(table: ForestYear): Board {
  const dims: GridDims = { rows: ROWS, cols: table.cols };
  const cells: BoardCell[] = [];
  for (let row = 0; row < dims.rows; row++) {
    for (let col = 0; col < dims.cols; col++) cells.push({ row, col, center: cellCenter({ row, col }), day: null, interactive: false });
  }
  const byDate = new Map<string, BoardCell>();
  for (const day of table.days) {
    const cell = cells[day.row * dims.cols + day.col];
    if (!cell) continue; // a damaged table: the day is not on the board
    cell.day = day;
    cell.interactive = day.date <= table.today;
    byDate.set(day.date, cell);
  }
  const order = cells.map((_, i) => i).sort((a, b) => {
    const ca = cells[a];
    const cb = cells[b];
    if (!ca || !cb) return 0;
    return ca.row + ca.col - (cb.row + cb.col) || ca.row - cb.row;
  });
  return { year: table.year, dims, cells, order, byDate, today: table.today, days: table.days };
}

export function cellAt(board: Board, c: Cell): BoardCell | null {
  if (c.row < 0 || c.col < 0 || c.row >= board.dims.rows || c.col >= board.dims.cols) return null;
  return board.cells[c.row * board.dims.cols + c.col] ?? null;
}

/** The day `step` places away in calendar order from `date` that can be selected; `null` at the ends of the year. */
export function neighbourDay(board: Board, date: string, step: number): string | null {
  const index = board.days.findIndex((day) => day.date === date);
  if (index < 0) return null;
  const target = board.days[index + step];
  return target && target.date <= board.today ? target.date : null;
}

/** The last day of the year that can be selected (today, or 31 December in a past year); `null` in a future year. */
export function lastSelectable(board: Board): string | null {
  for (let i = board.days.length - 1; i >= 0; i--) {
    const day = board.days[i];
    if (day && day.date <= board.today) return day.date;
  }
  return null;
}

/** Where each month starts: its number (1..12) and the column of its 1st day. */
export function monthStarts(board: Board): { month: number; col: number; date: string }[] {
  return board.days.filter((day) => day.date.endsWith("-01")).map((day) => ({ month: Number(day.date.slice(5, 7)), col: day.col, date: day.date }));
}

/** A date for each row (weekday) of the board: the first one found, to read the weekday name from. */
export function rowDates(board: Board): (string | null)[] {
  const dates: (string | null)[] = Array.from({ length: board.dims.rows }, () => null);
  for (const day of board.days) if (dates[day.row] === null) dates[day.row] = day.date;
  return dates;
}

/** The cells to bring into view for "Winter" / "Spring" / "Summer" / "Autumn": every day of the season, and for winter only
 * its January-February end (the winter at the start of the year, not the December at the end). */
export function seasonCells(board: Board, season: Season): Cell[] {
  const picked = board.days.filter((day) => day.season === season && (season !== "winter" || Number(day.date.slice(5, 7)) <= 6));
  return picked.map((day) => ({ row: day.row, col: day.col }));
}
