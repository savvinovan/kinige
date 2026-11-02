// The board as data: one entry per cell of the grid, in painter's order.

import {
  type Cell,
  type DayData,
  type FirstWeekday,
  type GridDims,
  type GridKind,
  type Season,
  type YmD,
  dateAtCell,
  gridDims,
  isoKey,
  seasonOfMonth,
} from "./calendar";
import { type Pt, cellCenter } from "./iso";

export interface CellInfo {
  row: number;
  col: number;
  /** World position of the cell centre. */
  center: Pt;
  /** `null`: a cell without a date (the 7 missing dates of grid A, the unused corners of grid B). */
  date: YmD | null;
  key: string | null;
  season: Season | null;
  data: DayData | null;
}

export interface Scene {
  grid: GridKind;
  year: number;
  first: FirstWeekday;
  dims: GridDims;
  /** Row-major: `cells[row * cols + col]`. */
  cells: CellInfo[];
  /** Indices into `cells`, back to front (rows + cols ascending). */
  order: number[];
  data: Map<string, DayData>;
}

export function buildScene(
  grid: GridKind,
  year: number,
  first: FirstWeekday,
  data: Map<string, DayData>,
  southern = false,
): Scene {
  const dims = gridDims(grid, year, first);
  const cells: CellInfo[] = [];
  for (let row = 0; row < dims.rows; row++) {
    for (let col = 0; col < dims.cols; col++) {
      const date = dateAtCell(grid, year, first, { row, col });
      const key = date ? isoKey(date) : null;
      cells.push({
        row,
        col,
        center: cellCenter({ row, col }),
        date,
        key,
        season: date ? seasonOfMonth(date.m, southern) : null,
        data: key ? (data.get(key) ?? null) : null,
      });
    }
  }
  const order = cells.map((_, i) => i).sort((a, b) => {
    const ca = cells[a];
    const cb = cells[b];
    if (!ca || !cb) return 0;
    return ca.row + ca.col - (cb.row + cb.col) || ca.row - cb.row;
  });
  return { grid, year, first, dims, cells, order, data };
}

export function cellAt(scene: Scene, c: Cell): CellInfo | null {
  if (c.row < 0 || c.col < 0 || c.row >= scene.dims.rows || c.col >= scene.dims.cols) return null;
  return scene.cells[c.row * scene.dims.cols + c.col] ?? null;
}
