// Isometric (2:1) projection of the grid, the camera and the hit-test by the inverse transform. No sprites involved:
// a click is a point → world → grid cell, so a tall tree never intercepts the click of a neighbour.

import type { Cell, GridDims } from "./calendar";

/** Tile size in world units; a tile is a 2:1 rhombus. */
export const TW = 64;
export const TH = 32;

export interface Pt {
  x: number;
  y: number;
}

export interface Camera {
  /** Pixels per world unit. */
  s: number;
  /** Screen position of the world origin (the top corner of cell (0, 0)). */
  tx: number;
  ty: number;
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

/** The cell under a screen point, or `null` outside the board. */
export function hitTest(cam: Camera, dims: GridDims, p: Pt): Cell | null {
  const { gc, gr } = worldToGrid(screenToWorld(cam, p));
  if (!Number.isFinite(gc) || !Number.isFinite(gr)) return null; // NaN would pass every range check below
  const col = Math.floor(gc);
  const row = Math.floor(gr);
  if (row < 0 || col < 0 || row >= dims.rows || col >= dims.cols) return null;
  return { row, col };
}

/** Bounding box of the whole board in world units. */
export function boardBounds(dims: GridDims): { x0: number; y0: number; x1: number; y1: number } {
  return {
    x0: -dims.rows * (TW / 2),
    x1: dims.cols * (TW / 2),
    y0: 0,
    y1: (dims.rows + dims.cols) * (TH / 2),
  };
}

/** Camera that fits `box` into a view of `w` × `h` pixels with room for trees above the board and a margin. */
export function fitCamera(
  box: { x0: number; y0: number; x1: number; y1: number },
  w: number,
  h: number,
  margin: number,
  treeRoom: number,
): Camera {
  const bw = box.x1 - box.x0;
  const bh = box.y1 - box.y0 + treeRoom;
  const s = Math.max(0.01, Math.min((w - 2 * margin) / bw, (h - 2 * margin) / bh));
  return {
    s,
    tx: w / 2 - ((box.x0 + box.x1) / 2) * s,
    ty: h / 2 - ((box.y0 - treeRoom + box.y1) / 2) * s,
  };
}

/** Zoom by `factor` keeping the world point under the screen point `p` fixed. */
export function zoomAt(cam: Camera, p: Pt, factor: number, sMin: number, sMax: number): Camera {
  const s = Math.min(sMax, Math.max(sMin, cam.s * factor));
  const k = s / cam.s;
  return { s, tx: p.x - (p.x - cam.tx) * k, ty: p.y - (p.y - cam.ty) * k };
}
