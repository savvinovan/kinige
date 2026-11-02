// The forest widget: state, input (pan, zoom, pinch, double click, keys), the card, the growth animation.
// Rendering is requested on demand (no idle loop): the canvas redraws only when something changed or animates.

import {
  type Cell,
  type DayData,
  type FirstWeekday,
  type GridKind,
  type YmD,
  dateAtCell,
  dayPosition,
  isoKey,
  stageOf,
  syntheticYear,
} from "./calendar";
import { type Chrome, type View, drawHover, drawScene, lodOf } from "./draw";
import {
  type Camera,
  type Pt,
  TW,
  boardBounds,
  fitCamera,
  hitTest,
  worldToScreen,
  zoomAt,
} from "./iso";
import { type Scene, buildScene, cellAt } from "./scene";

const YEAR = 2026;
/** The "today" of the demo: the tree that grows when a note is added. */
const TODAY: YmD = { y: YEAR, m: 10, d: 16 };
const MAX_CELL_PX = 280;
/** How far below the "whole year" scale the user can zoom out (reaches the dot level of detail in a narrow window). */
const MIN_ZOOM = 0.2;
const STAGE_NAMES = ["empty ground", "a sprout", "a sapling", "a young tree", "a tree", "a large tree", "a great tree"];

/** A rendered bitmap of the board with its camera and margins (CSS pixels). */
interface Layer {
  canvas: HTMLCanvasElement;
  cam: Camera;
  mx: number;
  my: number;
  version: number;
  dpr: number;
}

export class Forest {
  readonly canvas: HTMLCanvasElement;
  readonly ctx: CanvasRenderingContext2D;
  scene: Scene;
  view: View;
  w = 0;
  h = 0;
  dpr = 1;
  frozen = false; // the benchmark drives the frames itself
  /** Pan and zoom blit a cached bitmap of the board; it is re-rendered when the camera rests (see `drawCached`). */
  useCache = true;
  drawMs = 0;
  /** Number of frames drawn: the "no gratuitous motion" test checks that it stands still when nothing happens. */
  frames = 0;

  private data: Map<string, DayData>;
  private chrome: Chrome;
  private grid: GridKind = "A";
  private first: FirstWeekday = "mon";
  private raf = 0;
  /** Bumped by everything that changes the picture except the camera and the hover: the layer is re-rendered when it moves. */
  private version = 1;
  private layer: Layer | null = null;
  private settleTimer = 0;
  private settled = false;
  private camAnim: { from: Camera; to: Camera; start: number; dur: number } | null = null;
  private fitCam: Camera;
  /** Backing-store density cap, for the benchmark (a density-1 canvas is a quarter of the pixels on a HiDPI screen). */
  private maxDpr = Number.POSITIVE_INFINITY;
  private reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
  private pointers = new Map<number, Pt>();
  private drag: { x: number; y: number; cam: Camera; moved: boolean } | null = null;
  private pinch: { dist: number; cam: Camera } | null = null;

  constructor(
    canvas: HTMLCanvasElement,
    private card: HTMLElement,
    private live: HTMLElement,
    private info: HTMLElement,
  ) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("2d context unavailable");
    this.ctx = ctx;
    const css = getComputedStyle(document.documentElement);
    this.chrome = {
      background: css.getPropertyValue("--color-surface-background").trim() || "#141414",
      text: css.getPropertyValue("--color-text-secondary").trim() || "#dedede",
      accent: css.getPropertyValue("--color-accent-default").trim() || "#88c0d0",
      font: css.getPropertyValue("--font-family-sans").trim() || "sans-serif",
    };
    this.data = syntheticYear(YEAR);
    // Today starts as a three-stage tree so that the demo button has something to grow.
    this.data.set(isoKey(TODAY), { notes: 3, photos: 0, weight: 4, stage: stageOf(4), firstLine: "Synthetic note: today so far" });
    this.scene = buildScene(this.grid, YEAR, this.first, this.data);
    this.fitCam = { s: 1, tx: 0, ty: 0 };
    this.view = { cam: this.fitCam, selected: null, hover: null, today: TODAY, grow: null, now: 0, sprites: false };
    this.bind();
    new ResizeObserver(() => this.resize()).observe(canvas);
    this.resize();
  }

  // ---- public API (the page buttons, the tests and the benchmark use it) ----------------------------------------

  setGrid(grid: GridKind): void {
    this.grid = grid;
    this.rebuild();
  }

  setFirstWeekday(first: FirstWeekday): void {
    this.first = first;
    this.rebuild();
  }

  /** Draw detailed trees from pre-rendered bitmaps (see `View.sprites`). */
  setSprites(on: boolean): void {
    this.view.sprites = on;
    this.refresh();
  }

  get gridKind(): GridKind {
    return this.grid;
  }

  get firstWeekday(): FirstWeekday {
    return this.first;
  }

  /** The "whole year" view. */
  fitYear(animate = true): void {
    this.fitCam = this.computeFit();
    this.moveTo(this.fitCam, animate);
  }

  /** Zoom so that a tile is `cellPx` pixels wide, centred on a date. */
  zoomToDate(key: string, cellPx: number, animate = false): void {
    const cell = this.cellByKey(key);
    if (!cell) return;
    const s = this.clampScale(cellPx / TW);
    const c = cell.center;
    this.moveTo({ s, tx: this.w / 2 - c.x * s, ty: this.h / 2 - (c.y - 0.5 * TW) * s }, animate);
  }

  select(key: string | null): void {
    if (key === null) {
      this.view.selected = null;
    } else {
      const cell = this.cellByKey(key);
      if (!cell) return;
      this.view.selected = { row: cell.row, col: cell.col };
      this.announce(cell.row, cell.col);
      this.ensureVisible(cell.center);
    }
    this.refresh();
  }

  /** Shows the card of a date as if the pointer were over it (screenshots). */
  hover(key: string | null): void {
    if (key === null) return this.setHover(null, null);
    const cell = this.cellByKey(key);
    if (!cell) return;
    this.setHover({ row: cell.row, col: cell.col }, worldToScreen(this.view.cam, cell.center));
  }

  /** Raises today's weight to the next stage, which starts the growth animation (PRD §7.2: only when the stage grows). */
  addNoteToday(): boolean {
    const key = isoKey(TODAY);
    const d = this.data.get(key);
    if (!d || d.stage >= 6) return false;
    let w = d.weight;
    while (stageOf(w) === d.stage) w += 0.5;
    const next: DayData = { ...d, notes: d.notes + 1, weight: w, stage: stageOf(w) };
    this.data.set(key, next);
    const cell = this.cellByKey(key);
    if (cell) cell.data = next;
    this.view.grow = this.reduceMotion ? null : { key, from: d.stage, to: next.stage, start: performance.now(), dur: 700 };
    this.refresh();
    return true;
  }

  /** A date whose tree has tall neighbours in front of it: shows the semi-transparency of trees in front of the selection. */
  pickTransparencyDemo(): string | null {
    const { cells, dims } = this.scene;
    let best: { key: string; score: number } | null = null;
    for (const c of cells) {
      if (!c.key || !c.data || c.data.stage < 2 || c.data.stage > 4) continue;
      const a = cells[(c.row + 1) * dims.cols + c.col];
      const b = cells[c.row * dims.cols + c.col + 1];
      const score = (a?.data?.stage ?? 0) + (b?.data?.stage ?? 0);
      if (c.row + 1 < dims.rows && c.col + 1 < dims.cols && (!best || score > best.score)) best = { key: c.key, score };
    }
    return best?.key ?? null;
  }

  dateAt(p: Pt): string | null {
    const c = hitTest(this.view.cam, this.scene.dims, p);
    const cell = c && cellAt(this.scene, c);
    return cell?.key ?? null;
  }

  screenOf(key: string): Pt | null {
    const cell = this.cellByKey(key);
    return cell ? worldToScreen(this.view.cam, cell.center) : null;
  }

  invalidate(): void {
    if (this.frozen || this.raf) return;
    this.raf = requestAnimationFrame((t) => {
      this.raf = 0;
      this.frame(t);
    });
  }

  /** One frame: advances the camera animation, draws, and asks for another frame only while something animates. */
  frame(now: number): void {
    this.view.now = now;
    let again = false;
    if (this.camAnim) {
      const a = this.camAnim;
      const t = Math.min(1, (now - a.start) / a.dur);
      const e = 1 - (1 - t) ** 3;
      this.view.cam = {
        s: a.from.s * (a.to.s / a.from.s) ** e,
        tx: a.from.tx + (a.to.tx - a.from.tx) * e,
        ty: a.from.ty + (a.to.ty - a.from.ty) * e,
      };
      if (t >= 1) this.camAnim = null;
      else again = true;
    }
    this.frames++;
    const t0 = performance.now();
    if (this.view.grow) this.version++; // the growing tree changes every frame
    const animating = this.useCache ? this.drawCached() : drawScene(this.ctx, this.w, this.h, this.scene, this.view, this.chrome);
    drawHover(this.ctx, this.scene, this.view, this.chrome);
    if (animating) again = true;
    this.drawMs = performance.now() - t0;
    if (!again && this.view.grow && now - this.view.grow.start >= this.view.grow.dur) this.view.grow = null;
    this.updateInfo();
    if (again) this.invalidate();
  }

  /** Re-renders the picture (the fonts arrived, the data changed). */
  refresh(): void {
    this.version++;
    this.invalidate();
  }

  // ---- internals -----------------------------------------------------------------------------------------------

  /**
   * The board is static between data changes, so a pan or a zoom only moves a bitmap of it: the board is rendered into a
   * layer larger than the window (a quarter of the window of margin on each side) and every gesture frame is one
   * `drawImage`. The layer is re-rendered when the content changes, when the camera has rested for 140 ms (crisp again),
   * or when the view leaves the layer or the scale differs by more than 0.6..1.7 from the layer's. The hover outline is
   * drawn over the layer and does not invalidate it.
   */
  private drawCached(): boolean {
    const L = this.layer;
    const cam = this.view.cam;
    const same = L !== null && L.cam.s === cam.s && L.cam.tx === cam.tx && L.cam.ty === cam.ty;
    let animating = false;
    if (!L || L.version !== this.version || L.dpr !== this.dpr || (!same && (this.settled || !this.covers(L)))) {
      animating = this.renderLayer();
      this.settled = false;
    }
    window.clearTimeout(this.settleTimer);
    const cur = this.layer;
    if (!cur) return animating;
    if (cur.cam.s !== cam.s || cur.cam.tx !== cam.tx || cur.cam.ty !== cam.ty) {
      this.settleTimer = window.setTimeout(() => {
        this.settled = true;
        this.invalidate();
      }, 140);
    }
    this.blit(cur, cam);
    return animating;
  }

  /** Draws the part of the layer that lands in the window: a source rectangle, so the cost follows the window, not the layer. */
  private blit(L: Layer, cam: Camera): void {
    const k = cam.s / L.cam.s;
    const dx = (-L.mx - L.cam.tx) * k + cam.tx;
    const dy = (-L.my - L.cam.ty) * k + cam.ty;
    const dw = (L.canvas.width / L.dpr) * k;
    const dh = (L.canvas.height / L.dpr) * k;
    const x0 = Math.max(0, dx);
    const y0 = Math.max(0, dy);
    const x1 = Math.min(this.w, dx + dw);
    const y1 = Math.min(this.h, dy + dh);
    if (x1 <= x0 || y1 <= y0) return;
    const f = L.dpr / k;
    this.ctx.drawImage(L.canvas, (x0 - dx) * f, (y0 - dy) * f, (x1 - x0) * f, (y1 - y0) * f, x0, y0, x1 - x0, y1 - y0);
  }

  /** Does the layer cover the whole window at the current camera, at a scale close enough to look right? */
  private covers(L: Layer): boolean {
    const cam = this.view.cam;
    const k = cam.s / L.cam.s;
    if (k < 0.6 || k > 1.7) return false;
    // The window corners in the layer's screen space.
    const x0 = (0 - cam.tx) / k + L.cam.tx;
    const y0 = (0 - cam.ty) / k + L.cam.ty;
    const x1 = (this.w - cam.tx) / k + L.cam.tx;
    const y1 = (this.h - cam.ty) / k + L.cam.ty;
    return x0 >= -L.mx && y0 >= -L.my && x1 <= this.w + L.mx && y1 <= this.h + L.my;
  }

  private renderLayer(): boolean {
    const mx = Math.round(this.w * 0.25);
    const my = Math.round(this.h * 0.25);
    const W = this.w + 2 * mx;
    const H = this.h + 2 * my;
    const canvas = this.layer?.canvas ?? document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(W * this.dpr));
    canvas.height = Math.max(1, Math.round(H * this.dpr));
    const g = canvas.getContext("2d");
    if (!g) return false;
    g.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    const cam = this.view.cam;
    const shifted: View = { ...this.view, cam: { s: cam.s, tx: cam.tx + mx, ty: cam.ty + my } };
    const animating = drawScene(g, W, H, this.scene, shifted, this.chrome);
    this.layer = { canvas, cam, mx, my, version: this.version, dpr: this.dpr };
    return animating;
  }

  private rebuild(): void {
    this.scene = buildScene(this.grid, YEAR, this.first, this.data);
    this.version++;
    this.view.selected = null;
    this.view.hover = null;
    this.card.hidden = true;
    this.fitYear(false);
  }

  private cellByKey(key: string) {
    const [y, m, d] = key.split("-").map(Number);
    if (y === undefined || m === undefined || d === undefined) return null;
    const pos = dayPosition(this.grid, { y, m, d }, this.first);
    return pos ? cellAt(this.scene, pos) : null;
  }

  private resize(): void {
    const r = this.canvas.getBoundingClientRect();
    this.dpr = Math.min(window.devicePixelRatio || 1, this.maxDpr);
    this.w = Math.max(1, Math.round(r.width));
    this.h = Math.max(1, Math.round(r.height));
    this.canvas.width = Math.round(this.w * this.dpr);
    this.canvas.height = Math.round(this.h * this.dpr);
    this.ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    this.version++;
    const wasFit = this.view.cam === this.fitCam || this.fitCam.s === 1;
    this.fitCam = this.computeFit();
    if (wasFit) this.view.cam = this.fitCam;
    this.invalidate();
  }

  /** Caps the backing-store density; `Infinity` is the native one. */
  setMaxDpr(n: number): void {
    this.maxDpr = n;
    this.resize();
  }

  private computeFit(): Camera {
    const margin = 36;
    // Room above the board for the tallest tree, and the labels on the left edge.
    return fitCamera(boardBounds(this.scene.dims), this.w, this.h, margin, 1.6 * TW);
  }

  private clampScale(s: number): number {
    return Math.min(MAX_CELL_PX / TW, Math.max(this.fitCam.s * MIN_ZOOM, s));
  }

  private moveTo(cam: Camera, animate: boolean): void {
    cam = this.clampCam(cam);
    this.camAnim = animate && !this.reduceMotion ? { from: this.view.cam, to: cam, start: performance.now(), dur: 200 } : null;
    if (!this.camAnim) this.view.cam = cam;
    this.invalidate();
  }

  /** Keep at least a part of the board on screen. */
  private clampCam(cam: Camera): Camera {
    const b = boardBounds(this.scene.dims);
    const keep = 120;
    const x0 = b.x0 * cam.s + cam.tx;
    const x1 = b.x1 * cam.s + cam.tx;
    const y0 = b.y0 * cam.s + cam.ty;
    const y1 = b.y1 * cam.s + cam.ty;
    let { tx, ty } = cam;
    if (x1 < keep) tx += keep - x1;
    if (x0 > this.w - keep) tx -= x0 - (this.w - keep);
    if (y1 < keep) ty += keep - y1;
    if (y0 > this.h - keep) ty -= y0 - (this.h - keep);
    return { s: cam.s, tx, ty };
  }

  private zoomBy(p: Pt, factor: number): void {
    this.camAnim = null;
    const next = zoomAt(this.view.cam, p, factor, this.fitCam.s * MIN_ZOOM, MAX_CELL_PX / TW);
    this.view.cam = this.clampCam(next);
    this.hoverFrom(p);
    this.invalidate();
  }

  private ensureVisible(world: Pt): void {
    const p = worldToScreen(this.view.cam, world);
    const m = 60;
    let { tx, ty } = this.view.cam;
    if (p.x < m) tx += m - p.x;
    if (p.x > this.w - m) tx -= p.x - (this.w - m);
    if (p.y < m + TW * this.view.cam.s) ty += m + TW * this.view.cam.s - p.y;
    if (p.y > this.h - m) ty -= p.y - (this.h - m);
    if (tx !== this.view.cam.tx || ty !== this.view.cam.ty) this.moveTo({ ...this.view.cam, tx, ty }, true);
  }

  private local(e: { clientX: number; clientY: number }): Pt {
    const r = this.canvas.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  private bind(): void {
    const c = this.canvas;
    c.addEventListener("pointerdown", (e) => {
      c.setPointerCapture(e.pointerId);
      c.focus({ preventScroll: true });
      this.pointers.set(e.pointerId, this.local(e));
      if (this.pointers.size === 2) {
        const [a, b] = [...this.pointers.values()];
        if (a && b) this.pinch = { dist: Math.hypot(a.x - b.x, a.y - b.y), cam: this.view.cam };
        this.drag = null;
      } else {
        const p = this.local(e);
        this.drag = { x: p.x, y: p.y, cam: this.view.cam, moved: false };
      }
    });
    c.addEventListener("pointermove", (e) => {
      const p = this.local(e);
      if (this.pointers.has(e.pointerId)) this.pointers.set(e.pointerId, p);
      if (this.pinch && this.pointers.size === 2) {
        const [a, b] = [...this.pointers.values()];
        if (a && b) {
          const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
          const k = Math.hypot(a.x - b.x, a.y - b.y) / this.pinch.dist;
          this.view.cam = this.clampCam(zoomAt(this.pinch.cam, mid, k, this.fitCam.s * MIN_ZOOM, MAX_CELL_PX / TW));
          this.invalidate();
        }
        return;
      }
      if (this.drag) {
        const dx = p.x - this.drag.x;
        const dy = p.y - this.drag.y;
        if (!this.drag.moved && Math.hypot(dx, dy) > 4) {
          this.drag.moved = true;
          c.classList.add("dragging");
          this.setHover(null, null);
        }
        if (this.drag.moved) {
          this.camAnim = null;
          this.view.cam = this.clampCam({ ...this.drag.cam, tx: this.drag.cam.tx + dx, ty: this.drag.cam.ty + dy });
          this.invalidate();
        }
        return;
      }
      this.hoverFrom(p);
    });
    const end = (e: PointerEvent) => {
      const p = this.local(e);
      this.pointers.delete(e.pointerId);
      if (this.pointers.size < 2) this.pinch = null;
      if (this.drag && !this.drag.moved && e.type === "pointerup") {
        const key = this.dateAt(p);
        if (key) this.select(key);
      }
      this.drag = null;
      c.classList.remove("dragging");
    };
    c.addEventListener("pointerup", end);
    c.addEventListener("pointercancel", end);
    c.addEventListener("pointerleave", () => this.setHover(null, null));
    c.addEventListener("wheel", (e) => {
      e.preventDefault();
      const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1;
      const k = e.ctrlKey ? 0.01 : 0.0018; // ctrl + wheel is how Chromium reports a trackpad pinch
      this.zoomBy(this.local(e), Math.exp(-e.deltaY * unit * k));
    }, { passive: false });
    c.addEventListener("dblclick", (e) => {
      const p = this.local(e);
      this.zoomBy(p, 2);
    });
    // Safari / WKWebView report a trackpad pinch as gesture events.
    let gestureStart = 1;
    const anyC = c as unknown as EventTarget;
    anyC.addEventListener("gesturestart", (e) => {
      e.preventDefault();
      gestureStart = 1;
    });
    anyC.addEventListener("gesturechange", (e) => {
      e.preventDefault();
      const ge = e as unknown as { scale: number; clientX: number; clientY: number };
      this.zoomBy(this.local(ge), ge.scale / gestureStart);
      gestureStart = ge.scale;
    });
    c.addEventListener("keydown", (e) => this.onKey(e));
  }

  private hoverFrom(p: Pt): void {
    const cell = hitTest(this.view.cam, this.scene.dims, p);
    const info = cell && cellAt(this.scene, cell);
    this.setHover(info?.date ? cell : null, p);
  }

  private setHover(cell: Cell | null, p: Pt | null): void {
    const same = cell?.row === this.view.hover?.row && cell?.col === this.view.hover?.col;
    this.view.hover = cell;
    if (!cell || !p) {
      this.card.hidden = true;
      if (!same) this.invalidate();
      return;
    }
    const info = cellAt(this.scene, cell);
    if (info?.date && info.key) {
      const d = info.data;
      const when = new Date(Date.UTC(info.date.y, info.date.m - 1, info.date.d)).toLocaleDateString("en", {
        weekday: "short", month: "short", day: "numeric", year: "numeric", timeZone: "UTC",
      });
      const counts = d && d.notes > 0 ? `${d.notes} ${d.notes === 1 ? "note" : "notes"}${d.photos ? ` · ${d.photos} ${d.photos === 1 ? "photo" : "photos"}` : ""} · ${STAGE_NAMES[d.stage]}` : "No notes";
      this.card.replaceChildren();
      const strong = document.createElement("strong");
      strong.textContent = when;
      const span = document.createElement("span");
      span.textContent = counts;
      this.card.append(strong, span);
      if (d?.firstLine) {
        const line = document.createElement("p");
        line.textContent = d.firstLine;
        this.card.append(line);
      }
      this.card.hidden = false;
      const cw = this.card.offsetWidth;
      const ch = this.card.offsetHeight;
      const x = p.x + 16 + cw > this.w ? p.x - 16 - cw : p.x + 16;
      const y = p.y + 16 + ch > this.h ? p.y - 16 - ch : p.y + 16;
      this.card.style.left = `${Math.max(0, x)}px`;
      this.card.style.top = `${Math.max(0, y)}px`;
    }
    if (!same) this.invalidate();
  }

  /** ← → move by a day, ↑ ↓ by a month (A) or a week (B); non-existent cells are skipped. */
  private onKey(e: KeyboardEvent): void {
    const keys: Record<string, [number, number]> = {
      ArrowLeft: [0, -1],
      ArrowRight: [0, 1],
      ArrowUp: [-1, 0],
      ArrowDown: [1, 0],
    };
    const delta = keys[e.key];
    if (e.key === "Escape") {
      this.select(null);
      return;
    }
    if (!delta) return;
    e.preventDefault();
    const sel = this.view.selected ?? this.cellByKey(isoKey(TODAY));
    if (!sel) return;
    const next = this.step(sel, delta);
    if (next) this.select(isoKey(next));
  }

  private step(from: Cell, [dr, dc]: [number, number]): YmD | null {
    // Day axis: one step along the chronological order (±1 day). Other axis: ±1 month in A, ±7 days in B.
    const here = dateAtCell(this.grid, YEAR, this.first, from);
    if (!here) return null;
    const t = Date.UTC(here.y, here.m - 1, here.d);
    let target: Date;
    if (dc !== 0) {
      target = new Date(t + dc * 86_400_000);
    } else if (this.grid === "A") {
      const mdays = new Date(Date.UTC(here.y, here.m + dr, 0)).getUTCDate();
      target = new Date(Date.UTC(here.y, here.m - 1 + dr, Math.min(here.d, mdays)));
    } else {
      target = new Date(t + dr * 7 * 86_400_000);
    }
    if (target.getUTCFullYear() !== YEAR) return null;
    return { y: YEAR, m: target.getUTCMonth() + 1, d: target.getUTCDate() };
  }

  private announce(row: number, col: number): void {
    const info = cellAt(this.scene, { row, col });
    if (!info?.date) return;
    const month = new Date(Date.UTC(info.date.y, info.date.m - 1, 1)).toLocaleDateString("en", { month: "long", timeZone: "UTC" });
    const d = info.data;
    this.live.textContent = `${month} ${info.date.d}, ${d && d.notes > 0 ? `${d.notes} notes, ${STAGE_NAMES[d.stage]}` : "no notes"}`;
  }

  private updateInfo(): void {
    const cellPx = TW * this.view.cam.s;
    this.info.textContent = `${lodOf(this.view.cam)} · tile ${cellPx.toFixed(0)} px · ${Math.round(this.w)}×${Math.round(this.h)} @${this.dpr}x`;
  }

  /** For the tests: the extreme camera values. */
  get limits(): { fitScale: number; minScale: number; maxScale: number } {
    return { fitScale: this.fitCam.s, minScale: this.fitCam.s * MIN_ZOOM, maxScale: MAX_CELL_PX / TW };
  }
}
