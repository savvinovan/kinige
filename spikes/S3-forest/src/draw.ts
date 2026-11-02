// Canvas 2D renderer: ground tiles, procedural trees with three levels of detail, selection, transparency of the
// trees in front of the selected one, edge labels. Simple shapes only: no external assets.

import { type Camera, type Pt, TH, TW, worldToScreen } from "./iso";
import type { CellInfo, Scene } from "./scene";
import { type Season, type YmD, MAX_STAGE } from "./calendar";

export type Lod = "dot" | "simple" | "detailed";

/** On-screen tile width (px) below which a cell is a dot, and below which it is a simplified sprite. */
export const LOD_DOT_BELOW = 14;
export const LOD_SIMPLE_BELOW = 32;

export function lodOf(cam: Camera): Lod {
  const w = TW * cam.s;
  return w < LOD_DOT_BELOW ? "dot" : w < LOD_SIMPLE_BELOW ? "simple" : "detailed";
}

// PLACEHOLDER colours of the forest: spike-local until the art pipeline (PRD §7.5) produces real sprites.
// Everything around the canvas takes its colours from the design system tokens.
interface SeasonPalette {
  ground: [string, string];
  crown: { mid: string; dark: string; light: string };
  trunk: string;
  accent: string;
}
const PALETTE: Record<Season, SeasonPalette> = {
  winter: { ground: ["#c9d6e0", "#bccbd8"], crown: { mid: "#3f6b5c", dark: "#2a4a40", light: "#5a8a78" }, trunk: "#5b4636", accent: "#ffffff" },
  spring: { ground: ["#9fcf7a", "#92c46d"], crown: { mid: "#7fc46a", dark: "#5ea54e", light: "#a6df8e" }, trunk: "#6b5039", accent: "#f2a7c3" },
  summer: { ground: ["#6fb04f", "#63a447"], crown: { mid: "#3f8f3a", dark: "#2d6e2c", light: "#62b250" }, trunk: "#5b4430", accent: "#e8d44d" },
  autumn: { ground: ["#c9a45c", "#bb9650"], crown: { mid: "#d9822b", dark: "#b3541e", light: "#f0a94a" }, trunk: "#5b4430", accent: "#e8c547" },
};
const VOID_TILE = "#1d1d1d";

/** Chrome colours read from the design system tokens at start-up. */
export interface Chrome {
  background: string;
  text: string;
  accent: string;
  font: string;
}

export interface View {
  cam: Camera;
  selected: { row: number; col: number } | null;
  hover: { row: number; col: number } | null;
  today: YmD;
  /** The growth animation of today's tree. */
  grow: { key: string; from: number; to: number; start: number; dur: number } | null;
  /** The time of this frame (performance.now()). */
  now: number;
  /** Draw detailed trees from pre-rendered bitmaps (what an atlas of Blender sprites will cost) instead of vector shapes. */
  sprites: boolean;
}

// Height of the whole tree and radius of the crown in tile widths, per stage 0..6.
const HEIGHT = [0, 0.3, 0.55, 0.85, 1.2, 1.6, 2.05];
const RADIUS = [0, 0.08, 0.14, 0.2, 0.26, 0.32, 0.38];

function lerpStage(table: number[], f: number): number {
  const lo = Math.min(MAX_STAGE, Math.max(0, Math.floor(f)));
  const hi = Math.min(MAX_STAGE, lo + 1);
  const a = table[lo] ?? 0;
  const b = table[hi] ?? a;
  return a + (b - a) * (f - lo);
}

function ease(t: number): number {
  const u = 1 - Math.min(1, Math.max(0, t));
  return 1 - u * u * u;
}

/** The stage to draw for a cell: fractional while the growth animation runs. */
function shownStage(cell: CellInfo, view: View): number {
  const g = view.grow;
  if (g && cell.key === g.key) return g.from + (g.to - g.from) * ease((view.now - g.start) / g.dur);
  return cell.data?.stage ?? 0;
}

interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

function treeBox(c: Pt, f: number, s: number): Box {
  const r = lerpStage(RADIUS, f) * TW * s;
  const h = lerpStage(HEIGHT, f) * TW * s;
  return { x0: c.x - r * 1.2, x1: c.x + r * 1.2, y0: c.y - h, y1: c.y };
}

function overlaps(a: Box, b: Box): boolean {
  return a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0;
}

/** Draws a frame. Returns `true` while an animation is running and another frame is needed. */
export function drawScene(ctx: CanvasRenderingContext2D, w: number, h: number, scene: Scene, view: View, chrome: Chrome): boolean {
  const { cam } = view;
  const s = cam.s;
  const tw = TW * s;
  const th = TH * s;
  const lod = lodOf(cam);

  ctx.fillStyle = chrome.background;
  ctx.fillRect(0, 0, w, h);

  // Visible cells: a generous box, trees reach up to ~2 tiles above their tile.
  const screen = new Map<number, Pt>();
  const visible: number[] = [];
  for (const i of scene.order) {
    const c = scene.cells[i];
    if (!c) continue;
    const p = worldToScreen(cam, c.center);
    if (p.x < -tw || p.x > w + tw || p.y < -2.2 * tw || p.y > h + th) continue;
    screen.set(i, p);
    visible.push(i);
  }

  // Pass 1: ground.
  for (const i of visible) {
    const c = scene.cells[i];
    const p = screen.get(i);
    if (!c || !p) continue;
    const pal = c.season ? PALETTE[c.season] : null;
    ctx.fillStyle = pal ? pal.ground[(c.row + c.col) % 2] ?? pal.ground[0] : VOID_TILE;
    diamond(ctx, p, tw, th, lod === "dot" ? 0.92 : 1);
    ctx.fill();
  }

  // Today and the selection sit on the ground, under the trees; the hover outline is drawn over the picture by `drawHover`.
  const todayCell = scene.cells.find((c) => c.date && c.date.m === view.today.m && c.date.d === view.today.d);
  if (todayCell) outline(ctx, view, todayCell, chrome.text, 1, 0.55);
  const sel = view.selected && scene.cells[view.selected.row * scene.dims.cols + view.selected.col];
  if (sel) outline(ctx, view, sel, chrome.accent, Math.max(2, s * 3), 1, true);

  // Pass 2: trees, back to front. Those in front of the selected tree and overlapping it become semi-transparent.
  const selP = sel ? worldToScreen(cam, sel.center) : null;
  const selBox = sel && selP ? treeBox(selP, shownStage(sel, view), s) : null;
  let animating = false;
  for (const i of visible) {
    const c = scene.cells[i];
    const p = screen.get(i);
    if (!c || !p || !c.season) continue;
    const f = shownStage(c, view);
    if (view.grow && c.key === view.grow.key) animating = true;
    if (f <= 0) {
      if ((c.data?.weight ?? 0) > 0) tuft(ctx, p, s, c.season);
      continue;
    }
    const front = sel && selBox && c !== sel && c.row + c.col > sel.row + sel.col && overlaps(treeBox(p, f, s), selBox);
    const baked = view.sprites && lod === "detailed" && Number.isInteger(f) ? sprite(c.season, f) : null;
    if (front) drawGhost(ctx, p, f, s, c.season, lod, FRONT_ALPHA);
    else if (baked) drawSprite(ctx, baked, p, s);
    else drawTree(ctx, p, f, s, c.season, lod);
  }

  // The selection outline once more over the trees (thin): a selected tile hidden behind a canopy must still be findable.
  if (sel) outline(ctx, view, sel, chrome.accent, 1.5, 0.9);
  drawLabels(ctx, scene, view, chrome);
  return animating && view.now - (view.grow?.start ?? 0) < (view.grow?.dur ?? 0);
}

// ---- bitmap sprites: the cost model of the real art pipeline (PRD §7.5), where trees are atlas frames -------------

/** Sprites are baked at this world scale (a 128 px tile: sharp enough for a retina window at the whole-year zoom). */
const BAKE_SCALE = 2;

interface Sprite {
  canvas: HTMLCanvasElement;
  /** Position of the tree base (the tile centre) inside the bitmap, in bitmap pixels. */
  ox: number;
  oy: number;
}

const sprites = new Map<string, Sprite>();

function sprite(season: Season, stage: number): Sprite | null {
  const key = `${season}:${stage}`;
  const hit = sprites.get(key);
  if (hit) return hit;
  const R = lerpStage(RADIUS, stage) * TW * BAKE_SCALE;
  const H = lerpStage(HEIGHT, stage) * TW * BAKE_SCALE;
  const pad = Math.ceil(R * 0.5) + 6;
  const w = Math.ceil(2 * R * 1.6 + 2 * pad);
  const h = Math.ceil(H + R * 0.7 + 2 * pad);
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d");
  if (!g) return null;
  const made = { canvas, ox: w / 2, oy: h - pad - R * 0.4 };
  drawTree(g, { x: made.ox, y: made.oy }, stage, BAKE_SCALE, season, "detailed");
  sprites.set(key, made);
  return made;
}

function drawSprite(ctx: CanvasRenderingContext2D, sp: Sprite, p: Pt, s: number): void {
  const k = s / BAKE_SCALE;
  ctx.drawImage(sp.canvas, p.x - sp.ox * k, p.y - sp.oy * k, sp.canvas.width * k, sp.canvas.height * k);
}

/** Opacity of a tree standing in front of the selected one. */
const FRONT_ALPHA = 0.32;

let scratch: HTMLCanvasElement | null = null;

/**
 * A semi-transparent tree. Drawing its blobs one by one with `globalAlpha` would show every overlap as a darker ring, so
 * the tree is drawn opaque into a scratch canvas and that canvas is composited once with the alpha.
 */
function drawGhost(ctx: CanvasRenderingContext2D, p: Pt, f: number, s: number, season: Season, lod: Lod, alpha: number): void {
  const box = treeBox(p, f, s);
  const pad = 4 + lerpStage(RADIUS, f) * TW * s * 0.6;
  const x0 = Math.floor(box.x0 - pad);
  const y0 = Math.floor(box.y0 - pad);
  const w = Math.ceil(box.x1 - box.x0 + 2 * pad);
  const h = Math.ceil(box.y1 - box.y0 + 2 * pad);
  const k = ctx.getTransform().a; // device pixel ratio
  scratch ??= document.createElement("canvas");
  scratch.width = Math.max(1, Math.ceil(w * k));
  scratch.height = Math.max(1, Math.ceil(h * k));
  const g = scratch.getContext("2d");
  if (!g) return drawTree(ctx, p, f, s, season, lod);
  g.setTransform(k, 0, 0, k, 0, 0);
  drawTree(g, { x: p.x - x0, y: p.y - y0 }, f, s, season, lod);
  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalAlpha = alpha;
  ctx.drawImage(scratch, Math.round(x0 * k), Math.round(y0 * k));
  ctx.restore();
}

/** The outline of the hovered day, drawn over the (possibly cached) picture so that a hover never re-renders the board. */
export function drawHover(ctx: CanvasRenderingContext2D, scene: Scene, view: View, chrome: Chrome): void {
  const hov = view.hover && scene.cells[view.hover.row * scene.dims.cols + view.hover.col];
  if (hov?.date) outline(ctx, view, hov, chrome.text, 1.5, 0.9);
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

function outline(ctx: CanvasRenderingContext2D, view: View, c: CellInfo, color: string, width: number, alpha: number, fill = false): void {
  const p = worldToScreen(view.cam, c.center);
  diamond(ctx, p, TW * view.cam.s, TH * view.cam.s, 1);
  if (fill) {
    ctx.globalAlpha = 0.28;
    ctx.fillStyle = color;
    ctx.fill();
  }
  ctx.globalAlpha = alpha;
  ctx.strokeStyle = color;
  ctx.lineWidth = width;
  ctx.stroke();
  ctx.globalAlpha = 1;
}

function tuft(ctx: CanvasRenderingContext2D, p: Pt, s: number, season: Season): void {
  ctx.fillStyle = PALETTE[season].crown.dark;
  ctx.beginPath();
  ctx.arc(p.x, p.y, Math.max(1, 3 * s), 0, Math.PI * 2);
  ctx.fill();
}

function circle(ctx: CanvasRenderingContext2D, x: number, y: number, r: number): void {
  ctx.beginPath();
  ctx.arc(x, y, r, 0, Math.PI * 2);
  ctx.fill();
}

function drawTree(ctx: CanvasRenderingContext2D, p: Pt, f: number, s: number, season: Season, lod: Lod): void {
  const pal = PALETTE[season];
  const H = lerpStage(HEIGHT, f) * TW * s;
  const R = lerpStage(RADIUS, f) * TW * s;
  if (lod === "dot") {
    ctx.fillStyle = pal.crown.mid;
    circle(ctx, p.x, p.y - R * 0.6, Math.max(1.5, R * 1.1));
    return;
  }
  const trunkH = H * 0.38;
  const trunkW = Math.max(2, R * 0.34);
  if (lod === "simple") {
    ctx.fillStyle = pal.trunk;
    ctx.fillRect(p.x - trunkW / 2, p.y - trunkH, trunkW, trunkH);
    ctx.fillStyle = pal.crown.mid;
    circle(ctx, p.x, p.y - trunkH - (H - trunkH) * 0.5, Math.max(R, (H - trunkH) * 0.5));
    return;
  }
  // Detailed: shadow, tapered trunk, three overlapping blobs with light and dark sides, seasonal details.
  ctx.fillStyle = "rgba(0,0,0,0.22)";
  ctx.beginPath();
  ctx.ellipse(p.x + R * 0.25, p.y, R * 1.05, R * 0.5, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = pal.trunk;
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
  ctx.fillStyle = pal.crown.dark;
  for (const [x, y, r] of blobs) circle(ctx, x + R * 0.1, y + R * 0.12, r);
  ctx.fillStyle = pal.crown.mid;
  for (const [x, y, r] of blobs) circle(ctx, x, y, r * 0.94);
  ctx.fillStyle = pal.crown.light;
  circle(ctx, p.x - R * 0.28, p.y - H + R * 0.55, R * 0.34);
  if (season === "winter") {
    ctx.fillStyle = pal.accent;
    ctx.beginPath();
    ctx.ellipse(p.x + R * 0.1, p.y - H + R * 0.22, R * 0.5, R * 0.2, 0, 0, Math.PI * 2);
    ctx.fill();
  } else if (season === "spring") {
    ctx.fillStyle = pal.accent;
    circle(ctx, p.x - R * 0.35, by, Math.max(1, R * 0.1));
    circle(ctx, p.x + R * 0.45, by - ch * 0.1, Math.max(1, R * 0.1));
    circle(ctx, p.x + R * 0.05, p.y - H + R * 0.9, Math.max(1, R * 0.09));
  } else if (season === "autumn") {
    ctx.fillStyle = pal.accent;
    circle(ctx, p.x + R * 0.7, p.y + R * 0.1, Math.max(1, R * 0.09));
    circle(ctx, p.x - R * 0.8, p.y + R * 0.18, Math.max(1, R * 0.08));
  }
}

// ---- labels ------------------------------------------------------------------------------------------------------

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS_MON = ["M", "T", "W", "T", "F", "S", "S"];

/** Hints on the board edges: months and day numbers for A; weekdays and months for B. They are what makes a date readable. */
function drawLabels(ctx: CanvasRenderingContext2D, scene: Scene, view: View, chrome: Chrome): void {
  const { cam } = view;
  const size = 11;
  ctx.font = `${size}px ${chrome.font}`;
  ctx.fillStyle = chrome.text;
  // A halo in the background colour keeps the labels readable over the canopies.
  ctx.strokeStyle = chrome.background;
  ctx.lineWidth = 3;
  ctx.lineJoin = "round";
  ctx.textBaseline = "middle";
  const put = (text: string, row: number, col: number, align: CanvasTextAlign) => {
    const c = { x: (col - row) * (TW / 2), y: (col + row + 1) * (TH / 2) };
    const p = worldToScreen(cam, c);
    ctx.textAlign = align;
    ctx.strokeText(text, p.x, p.y);
    ctx.fillText(text, p.x, p.y);
  };
  const coarse = TW * cam.s < 22;
  if (scene.grid === "A") {
    for (let r = 0; r < 12; r++) put(MONTHS[r] ?? "", r, -1, "right");
    for (let c = 0; c < 31; c++) if (!coarse || c % 5 === 4 || c === 0) put(String(c + 1), -1, c, "left");
  } else {
    const firstIdx = scene.first === "mon" ? 0 : 6;
    for (let r = 0; r < 7; r++) {
      const wd = (firstIdx + r) % 7;
      if (!coarse || r % 2 === 0) put(WEEKDAYS_MON[wd] ?? "", r, -1, "right");
    }
    for (let m = 1; m <= 12; m++) {
      const cell = scene.cells.find((c) => c.date && c.date.m === m && c.date.d === 1);
      if (cell) put(MONTHS[m - 1] ?? "", -1, cell.col, "left");
    }
  }
  ctx.globalAlpha = 1;
}
