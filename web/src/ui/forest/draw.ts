// Canvas 2D renderer of the board: ground tiles, placeholder trees (shapes.ts), the selection, the transparency of the trees
// in front of the selected one, the labels on the edges. No external assets; every colour comes from the palette (tokens).

import type { Season } from "../../api/generated/Season";
import type { Board, BoardCell } from "./board";
import { monthStarts, rowDates } from "./board";
import { type Camera, type Cell, type Pt, TH, TW, worldToScreen } from "./iso";
import type { Palette } from "./palette";
import { type Lod, drawTree, lodOfTile, overlaps, treeBox, treePad, tuft } from "./shapes";

export interface Grow {
  /** The date of the tree that grows. */
  date: string;
  from: number;
  to: number;
  /** `performance.now()` at the start, and the duration in ms. */
  start: number;
  dur: number;
  /** Easing of the progress 0..1. */
  ease: (t: number) => number;
}

export interface View {
  cam: Camera;
  selected: Cell | null;
  hover: Cell | null;
  grow: Grow | null;
  /** The time of this frame (`performance.now()`). */
  now: number;
}

/** Texts drawn on the board edges, in the interface language. */
export interface Labels {
  /** Short month names, January first. */
  months: readonly string[];
  /** One letter per month, for boards too small for the short names. */
  monthsNarrow: readonly string[];
  /** One letter per row of the board (the weekday of that row). */
  weekdays: readonly string[];
}

export function lodOf(cam: Camera): Lod {
  return lodOfTile(TW * cam.s);
}

/** The stage to draw for a cell: fractional while the growth animation runs. */
function shownStage(cell: BoardCell, view: View): number {
  const g = view.grow;
  if (g && cell.day?.date === g.date) return g.from + (g.to - g.from) * g.ease((view.now - g.start) / g.dur);
  return cell.day?.stage ?? 0;
}

/** Draws a frame. Returns `true` while the growth animation is running and another frame is needed. */
export function drawScene(ctx: CanvasRenderingContext2D, w: number, h: number, board: Board, view: View, palette: Palette, labels: Labels): boolean {
  const { cam } = view;
  const s = cam.s;
  const tw = TW * s;
  const th = TH * s;
  const lod = lodOf(cam);

  ctx.fillStyle = palette.background;
  ctx.fillRect(0, 0, w, h);

  // Visible cells: a generous box, trees reach up to ~2 tiles above their tile.
  const screen = new Map<number, Pt>();
  const visible: number[] = [];
  for (const i of board.order) {
    const c = board.cells[i];
    if (!c) continue;
    const p = worldToScreen(cam, c.center);
    if (p.x < -tw || p.x > w + tw || p.y < -2.2 * tw || p.y > h + th) continue;
    screen.set(i, p);
    visible.push(i);
  }

  // Pass 1: ground.
  for (const i of visible) {
    const c = board.cells[i];
    const p = screen.get(i);
    if (!c || !p) continue;
    const colors = c.day ? palette.seasons[c.day.season] : null;
    ctx.fillStyle = colors ? ((c.row + c.col) % 2 === 0 ? colors.groundA : colors.groundB) : palette.voidTile;
    diamond(ctx, p, tw, th, lod === "dot" ? 0.92 : 1);
    ctx.fill();
  }

  // Today and the selection sit on the ground, under the trees; the hover outline is drawn over the picture by `drawHover`.
  const todayCell = board.byDate.get(board.today);
  if (todayCell) outline(ctx, view, todayCell, palette.outlineToday, 1, 0.55);
  const sel = view.selected && board.cells[view.selected.row * board.dims.cols + view.selected.col];
  if (sel) outline(ctx, view, sel, palette.outline, Math.max(2, s * 3), 1, palette.selectionFill);

  // Pass 2: trees, back to front. Those in front of the selected tree and overlapping it become semi-transparent.
  const selP = sel ? worldToScreen(cam, sel.center) : null;
  const selBox = sel && selP ? treeBox(selP, shownStage(sel, view), s) : null;
  let animating = false;
  for (const i of visible) {
    const c = board.cells[i];
    const p = screen.get(i);
    if (!c || !p || !c.day || !c.interactive) continue;
    const colors = palette.seasons[c.day.season];
    const f = shownStage(c, view);
    if (view.grow && c.day.date === view.grow.date) animating = true;
    if (f <= 0) {
      if (c.day.notes > 0) tuft(ctx, p, s, colors);
      continue;
    }
    const front = sel && selBox && c !== sel && c.row + c.col > sel.row + sel.col && overlaps(treeBox(p, f, s), selBox);
    if (front) drawGhost(ctx, p, f, s, palette, c.day.season, lod, palette.frontAlpha);
    else drawTree(ctx, p, f, s, colors, palette.shadow, lod, c.day.season);
  }

  // The selection outline once more over the trees (thin): a selected tile hidden behind a canopy must still be findable.
  if (sel) outline(ctx, view, sel, palette.outline, 1.5, 0.9);
  drawLabels(ctx, board, view, palette, labels);
  return animating && view.now - (view.grow?.start ?? 0) < (view.grow?.dur ?? 0);
}

let scratch: HTMLCanvasElement | null = null;

/**
 * A semi-transparent tree. Drawing its blobs one by one with `globalAlpha` would show every overlap as a darker ring, so
 * the tree is drawn opaque into a scratch canvas and that canvas is composited once with the alpha.
 */
function drawGhost(ctx: CanvasRenderingContext2D, p: Pt, f: number, s: number, palette: Palette, season: Season, lod: Lod, alpha: number): void {
  const colors = palette.seasons[season];
  const box = treeBox(p, f, s);
  const pad = treePad(f, s);
  const x0 = Math.floor(box.x0 - pad);
  const y0 = Math.floor(box.y0 - pad);
  const w = Math.ceil(box.x1 - box.x0 + 2 * pad);
  const h = Math.ceil(box.y1 - box.y0 + 2 * pad);
  const k = ctx.getTransform().a; // device pixel ratio
  scratch ??= document.createElement("canvas");
  scratch.width = Math.max(1, Math.ceil(w * k));
  scratch.height = Math.max(1, Math.ceil(h * k));
  const g = scratch.getContext("2d");
  if (!g) return drawTree(ctx, p, f, s, colors, palette.shadow, lod, season);
  g.setTransform(k, 0, 0, k, 0, 0);
  drawTree(g, { x: p.x - x0, y: p.y - y0 }, f, s, colors, palette.shadow, lod, season);
  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalAlpha = alpha;
  ctx.drawImage(scratch, Math.round(x0 * k), Math.round(y0 * k));
  ctx.restore();
}

/** The outline of the hovered day, drawn over the (possibly cached) picture so that a hover never re-renders the board. */
export function drawHover(ctx: CanvasRenderingContext2D, board: Board, view: View, palette: Palette): void {
  const hov = view.hover && board.cells[view.hover.row * board.dims.cols + view.hover.col];
  if (hov?.interactive) outline(ctx, view, hov, palette.outlineHover, 1.5, 0.9);
}

function diamond(ctx: CanvasRenderingContext2D, p: Pt, tw: number, th: number, k: number): void {
  const hw = (tw / 2) * k;
  const hh = (th / 2) * k;
  ctx.beginPath();
  ctx.moveTo(p.x, p.y - hh);
  ctx.lineTo(p.x + hw, p.y);
  ctx.lineTo(p.x, p.y + hh);
  ctx.lineTo(p.x - hw, p.y);
  ctx.closePath();
}

function outline(ctx: CanvasRenderingContext2D, view: View, c: BoardCell, color: string, width: number, alpha: number, fillAlpha = 0): void {
  const p = worldToScreen(view.cam, c.center);
  diamond(ctx, p, TW * view.cam.s, TH * view.cam.s, 1);
  if (fillAlpha > 0) {
    ctx.globalAlpha = fillAlpha;
    ctx.fillStyle = color;
    ctx.fill();
  }
  ctx.globalAlpha = alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = width;
  ctx.stroke();
  ctx.globalAlpha = 1;
}

/** Below this tile width the months are one letter and the weekday letters are gone; below the next one the weekdays are thinned. */
const LABELS_COMPACT_BELOW = 24;
const LABELS_THIN_BELOW = 32;

/** Months on the front edge and weekday letters on the left edge: they are what makes a date readable. */
function drawLabels(ctx: CanvasRenderingContext2D, board: Board, view: View, palette: Palette, labels: Labels): void {
  const { cam } = view;
  const tile = TW * cam.s;
  if (tile < 12) return; // a dot-sized board: the labels would only smear
  const compact = tile < LABELS_COMPACT_BELOW; // a small board: one letter per month, no weekdays, or they would pile up
  const thin = tile < LABELS_THIN_BELOW;
  ctx.font = palette.font;
  ctx.fillStyle = palette.label;
  // A halo in the background colour keeps the labels readable over the canopies.
  ctx.strokeStyle = palette.labelHalo;
  ctx.lineWidth = 3;
  ctx.lineJoin = "round";
  ctx.textBaseline = "middle";
  const put = (text: string, row: number, col: number, align: CanvasTextAlign) => {
    const p = worldToScreen(cam, { x: (col - row) * (TW / 2), y: (col + row + 1) * (TH / 2) });
    ctx.textAlign = align;
    ctx.strokeText(text, p.x, p.y);
    ctx.fillText(text, p.x, p.y);
  };
  const dates = rowDates(board);
  for (let row = 0; row < board.dims.rows && !compact; row++) {
    const letter = labels.weekdays[row];
    if (letter && dates[row] !== null && (!thin || row % 2 === 0)) put(letter, row, -1, "right");
  }
  for (const { month, col } of monthStarts(board)) {
    const name = (compact ? labels.monthsNarrow : labels.months)[month - 1];
    if (name) put(name, board.dims.rows, col, "center");
  }
}
