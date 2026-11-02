// Geometry of the board only (ADR-0013): the isometric 2:1 projection of the grid, the camera, and the hit-test by the
// inverse transform. Which date sits in which cell is decided in Rust and arrives in the year table; nothing here knows a
// calendar. A click is point -> world -> cell, never sprites, so a tall tree does not take the click of its neighbour.

/** Tile size in world units; a tile is a 2:1 rhombus. */
export const TW = 64;
export const TH = 32;

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

/** World position of the centre of a cell. Columns run down-right, rows run down-left. */
export function cellCenter({ row, col }: Cell): Pt {
  return { x: (col - row) * (TW / 2), y: (col + row + 1) * (TH / 2) };
}

/** Fractional grid coordinates (col, row) of a world point: the inverse transform. */
export function worldToGrid(p: Pt): { gc: number; gr: number } {
  const u = p.x / (TW / 2);
  const v = p.y / (TH / 2);
  return { gc: (u + v) / 2, gr: (v - u) / 2 };
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
  return { x0: -dims.rows * (TW / 2), x1: dims.cols * (TW / 2), y0: 0, y1: (dims.rows + dims.cols) * (TH / 2) };
}

/** Bounding box of some cells (their rhombi) in world units; `null` for no cells. */
export function cellsBounds(cells: readonly Cell[]): Box | null {
  let box: Box | null = null;
  for (const cell of cells) {
    const c = cellCenter(cell);
    const next = { x0: c.x - TW / 2, x1: c.x + TW / 2, y0: c.y - TH / 2, y1: c.y + TH / 2 };
    box = box
      ? { x0: Math.min(box.x0, next.x0), x1: Math.max(box.x1, next.x1), y0: Math.min(box.y0, next.y0), y1: Math.max(box.y1, next.y1) }
      : next;
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
