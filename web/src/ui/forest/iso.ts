// Geometry of the board only (ADR-0013): the oblique projection of the grid, the camera, and the hit-test by the inverse
// transform. Which date sits in which cell is decided in Rust and arrives in the year table; nothing here knows a calendar.
// A click is point -> world -> cell, never sprites, so a tall tree does not take the click of its neighbour.
//
// The grid is drawn as a ground tilted away from the viewer, rotated so that the 53 weeks run along the horizontal axis of the
// screen (the first drawing, a 45-degree isometry, laid the year along a diagonal and used a third of a wide pane). A cell is a
// parallelogram: one step to the next week is `COL`, one step to the next weekday is `ROW`; trees stand upright on their cell.

/** One week to the right, in world units: also the width of a tile, which decides the level of detail. */
const COL = { x: 64, y: 10 };
/** One weekday down: it goes down and a little to the left, so that the rows do not fall on top of each other. */
const ROW = { x: -20, y: 26 };
export const TW = COL.x;
const DET = COL.x * ROW.y - ROW.x * COL.y;

export interface Pt {
  x: number;
  y: number;
}

export interface Cell {
  row: number;
  col: number;
}

export interface GridDims {
  rows: number;
  cols: number;
}

export interface Camera {
  /** Pixels per world unit. */
  s: number;
  /** Screen position of the world origin (the top corner of cell (0, 0)). */
  tx: number;
  ty: number;
}

export interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** World position of a point of the grid given in fractional (column, row) coordinates. */
function gridToWorld(gc: number, gr: number): Pt {
  return { x: gc * COL.x + gr * ROW.x, y: gc * COL.y + gr * ROW.y };
}

/** World position of the centre of a cell. The columns run to the right, the rows run down. */
export function cellCenter({ row, col }: Cell): Pt {
  return gridToWorld(col + 0.5, row + 0.5);
}

/** The four corners of a tile around its centre, in world units, in drawing order. */
export const TILE_CORNERS: readonly Pt[] = [
  { x: -(COL.x + ROW.x) / 2, y: -(COL.y + ROW.y) / 2 },
  { x: (COL.x - ROW.x) / 2, y: (COL.y - ROW.y) / 2 },
  { x: (COL.x + ROW.x) / 2, y: (COL.y + ROW.y) / 2 },
  { x: (ROW.x - COL.x) / 2, y: (ROW.y - COL.y) / 2 },
];

/** Fractional grid coordinates (col, row) of a world point: the inverse transform. */
export function worldToGrid(p: Pt): { gc: number; gr: number } {
  return { gc: (p.x * ROW.y - p.y * ROW.x) / DET, gr: (COL.x * p.y - COL.y * p.x) / DET };
}

export function worldToScreen(cam: Camera, p: Pt): Pt {
  return { x: p.x * cam.s + cam.tx, y: p.y * cam.s + cam.ty };
}

export function screenToWorld(cam: Camera, p: Pt): Pt {
  return { x: (p.x - cam.tx) / cam.s, y: (p.y - cam.ty) / cam.s };
}

/** The cell under a screen point, or `null` outside the board. A `NaN` or infinite point is a miss. */
export function hitTest(cam: Camera, dims: GridDims, p: Pt): Cell | null {
  // A camera that is not a camera (NaN, infinite, zero or negative scale) hits nothing instead of a wrong cell.
  if (!Number.isFinite(cam.s) || !Number.isFinite(cam.tx) || !Number.isFinite(cam.ty) || cam.s <= 0) return null;
  const { gc, gr } = worldToGrid(screenToWorld(cam, p));
  if (!Number.isFinite(gc) || !Number.isFinite(gr)) return null; // NaN would pass every range check below
  const col = Math.floor(gc);
  const row = Math.floor(gr);
  if (row < 0 || col < 0 || row >= dims.rows || col >= dims.cols) return null;
  return { row, col };
}

/** Bounding box of the whole board in world units. */
export function boardBounds(dims: GridDims): Box {
  const corners = [gridToWorld(0, 0), gridToWorld(dims.cols, 0), gridToWorld(0, dims.rows), gridToWorld(dims.cols, dims.rows)];
  return {
    x0: Math.min(...corners.map((c) => c.x)),
    x1: Math.max(...corners.map((c) => c.x)),
    y0: Math.min(...corners.map((c) => c.y)),
    y1: Math.max(...corners.map((c) => c.y)),
  };
}

/** Bounding box of some cells (their parallelograms) in world units; `null` for no cells. */
export function cellsBounds(cells: readonly Cell[]): Box | null {
  let box: Box | null = null;
  for (const cell of cells) {
    const c = cellCenter(cell);
    for (const corner of TILE_CORNERS) {
      const x = c.x + corner.x;
      const y = c.y + corner.y;
      box = box ? { x0: Math.min(box.x0, x), x1: Math.max(box.x1, x), y0: Math.min(box.y0, y), y1: Math.max(box.y1, y) } : { x0: x, x1: x, y0: y, y1: y };
    }
  }
  return box;
}

/** Camera that fits `box` into a view of `w` x `h` pixels with room for trees above the board and a margin. */
export function fitCamera(box: Box, w: number, h: number, margin: number, treeRoom: number): Camera {
  const bw = box.x1 - box.x0;
  const bh = box.y1 - box.y0 + treeRoom;
  const s = Math.max(0.01, Math.min((w - 2 * margin) / bw, (h - 2 * margin) / bh));
  return { s, tx: w / 2 - ((box.x0 + box.x1) / 2) * s, ty: h / 2 - ((box.y0 - treeRoom + box.y1) / 2) * s };
}

/** Zoom by `factor` keeping the world point under the screen point `p` fixed. */
export function zoomAt(cam: Camera, p: Pt, factor: number, sMin: number, sMax: number): Camera {
  const s = Math.min(sMax, Math.max(sMin, cam.s * factor));
  const k = s / cam.s;
  return { s, tx: p.x - (p.x - cam.tx) * k, ty: p.y - (p.y - cam.ty) * k };
}

/** Keeps at least `keep` pixels of the board on screen so that a drag can never lose it. */
export function clampCamera(cam: Camera, box: Box, w: number, h: number, keep: number): Camera {
  const x0 = box.x0 * cam.s + cam.tx;
  const x1 = box.x1 * cam.s + cam.tx;
  const y0 = box.y0 * cam.s + cam.ty;
  const y1 = box.y1 * cam.s + cam.ty;
  let { tx, ty } = cam;
  if (x1 < keep) tx += keep - x1;
  if (x0 > w - keep) tx -= x0 - (w - keep);
  if (y1 < keep) ty += keep - y1;
  if (y0 > h - keep) ty -= y0 - (h - keep);
  return { s: cam.s, tx, ty };
}
