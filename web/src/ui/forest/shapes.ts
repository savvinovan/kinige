// PLACEHOLDER shapes of the trees (slice 6): circles and trunks in three levels of detail. All the shape geometry lives in
// this one file; slice 9 replaces it with atlases from the art pipeline (PRD §7.5) and nothing else has to change. Colours
// are not here: they come from the season colours of the palette (`color.forest.*` tokens).

import type { Season } from "../../api/generated/Season";
import type { SeasonColors } from "./palette";
import { type Box, type Pt, TW } from "./iso";

export type Lod = "dot" | "simple" | "detailed";

/** On-screen tile width (px) below which a cell is a dot, and below which it is a simplified shape. */
export const LOD_DOT_BELOW = 14;
export const LOD_SIMPLE_BELOW = 32;

export const MAX_STAGE = 6;

export function lodOfTile(tilePx: number): Lod {
  return tilePx < LOD_DOT_BELOW ? "dot" : tilePx < LOD_SIMPLE_BELOW ? "simple" : "detailed";
}

// Height of the whole tree and radius of the crown in tile widths, per stage 0..6. A week is only about 17 px wide when the
// whole year is on screen, so the trees are drawn larger than their tile (a stage-6 crown is about one and a half tiles wide):
// neighbours overlap a little, which is what makes a forest of 53 columns readable.
const HEIGHT = [0, 0.45, 0.8, 1.3, 1.8, 2.4, 3.1];
const RADIUS = [0, 0.12, 0.2, 0.3, 0.39, 0.48, 0.57];

/** Height of the tallest tree in tile widths: the board needs this much room above its back edge. */
export const TALLEST = HEIGHT[MAX_STAGE] ?? 2;

/** Linear interpolation between the stages of a table: a fractional stage is a tree in the middle of its growth. */
function lerpStage(table: number[], f: number): number {
  const lo = Math.min(MAX_STAGE, Math.max(0, Math.floor(f)));
  const hi = Math.min(MAX_STAGE, lo + 1);
  const a = table[lo] ?? 0;
  const b = table[hi] ?? a;
  return a + (b - a) * (f - lo);
}

/** Bounding box of a tree of (fractional) stage `f` standing at `c`, for the overlap test of the selection. */
export function treeBox(c: Pt, f: number, s: number): Box {
  const r = lerpStage(RADIUS, f) * TW * s;
  const h = lerpStage(HEIGHT, f) * TW * s;
  return { x0: c.x - r * 1.2, x1: c.x + r * 1.2, y0: c.y - h, y1: c.y };
}

export function overlaps(a: Box, b: Box): boolean {
  return a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0;
}

/** How far a tree reaches from its base, for the scratch canvas of a semi-transparent tree. */
export function treePad(f: number, s: number): number {
  return 4 + lerpStage(RADIUS, f) * TW * s * 0.6;
}

/** The small tuft on a day that has notes but no growth yet (stage 0). */
export function tuft(ctx: CanvasRenderingContext2D, p: Pt, s: number, colors: SeasonColors): void {
  ctx.fillStyle = colors.crownDark;
  ctx.beginPath();
  ctx.arc(p.x, p.y, Math.max(1, 3 * s), 0, Math.PI * 2);
  ctx.fill();
}

function circle(ctx: CanvasRenderingContext2D, x: number, y: number, r: number): void {
  ctx.beginPath();
  ctx.arc(x, y, r, 0, Math.PI * 2);
  ctx.fill();
}

/** One tree of (fractional) stage `f` at `p` in the colours of its season. */
export function drawTree(
  ctx: CanvasRenderingContext2D,
  p: Pt,
  f: number,
  s: number,
  colors: SeasonColors,
  shadow: string,
  lod: Lod,
  detail: Season,
): void {
  const H = lerpStage(HEIGHT, f) * TW * s;
  const R = lerpStage(RADIUS, f) * TW * s;
  if (lod === "dot") {
    ctx.fillStyle = colors.crownMid;
    circle(ctx, p.x, p.y - R * 0.6, Math.max(1.5, R * 1.1));
    return;
  }
  const trunkH = H * 0.38;
  const trunkW = Math.max(2, R * 0.34);
  if (lod === "simple") {
    ctx.fillStyle = colors.trunk;
    ctx.fillRect(p.x - trunkW / 2, p.y - trunkH, trunkW, trunkH);
    ctx.fillStyle = colors.crownMid;
    circle(ctx, p.x, p.y - trunkH - (H - trunkH) * 0.5, Math.max(R, (H - trunkH) * 0.5));
    return;
  }
  // Detailed: shadow, tapered trunk, three overlapping blobs with a light and a dark side, a seasonal detail.
  ctx.fillStyle = shadow;
  ctx.beginPath();
  ctx.ellipse(p.x + R * 0.25, p.y, R * 1.05, R * 0.5, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = colors.trunk;
  ctx.beginPath();
  ctx.moveTo(p.x - trunkW * 0.65, p.y);
  ctx.lineTo(p.x - trunkW * 0.4, p.y - trunkH);
  ctx.lineTo(p.x + trunkW * 0.4, p.y - trunkH);
  ctx.lineTo(p.x + trunkW * 0.65, p.y);
  ctx.closePath();
  ctx.fill();
  const ch = H - trunkH;
  const by = p.y - trunkH - ch * 0.38;
  const blobs: [number, number, number][] = [
    [p.x, by, R],
    [p.x - R * 0.5, by - ch * 0.18, R * 0.72],
    [p.x + R * 0.12, p.y - H + R * 0.62, R * 0.64],
  ];
  ctx.fillStyle = colors.crownDark;
  for (const [x, y, r] of blobs) circle(ctx, x + R * 0.1, y + R * 0.12, r);
  ctx.fillStyle = colors.crownMid;
  for (const [x, y, r] of blobs) circle(ctx, x, y, r * 0.94);
  ctx.fillStyle = colors.crownLight;
  circle(ctx, p.x - R * 0.28, p.y - H + R * 0.55, R * 0.34);
  ctx.fillStyle = colors.accent;
  if (detail === "winter") {
    ctx.beginPath();
    ctx.ellipse(p.x + R * 0.1, p.y - H + R * 0.22, R * 0.5, R * 0.2, 0, 0, Math.PI * 2);
    ctx.fill();
  } else if (detail === "spring") {
    circle(ctx, p.x - R * 0.35, by, Math.max(1, R * 0.1));
    circle(ctx, p.x + R * 0.45, by - ch * 0.1, Math.max(1, R * 0.1));
    circle(ctx, p.x + R * 0.05, p.y - H + R * 0.9, Math.max(1, R * 0.09));
  } else if (detail === "autumn") {
    circle(ctx, p.x + R * 0.7, p.y + R * 0.1, Math.max(1, R * 0.09));
    circle(ctx, p.x - R * 0.8, p.y + R * 0.18, Math.max(1, R * 0.08));
  }
}
