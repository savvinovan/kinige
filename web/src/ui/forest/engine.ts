// The board engine: camera, input (pan, zoom, pinch, double click), the layer cache and the growth animation, around the
// drawing in draw.ts. Frames are requested on demand: when nothing changes and nothing animates, no frame is drawn.

import { type Board, cellAt } from "./board";
import { type Grow, type Labels, type View, drawHover, drawScene } from "./draw";
import { cubicBezier, parseDuration } from "./easing";
import { type Camera, type Cell, type Pt, TW, boardBounds, cellsBounds, clampCamera, fitCamera, hitTest, worldToScreen, zoomAt } from "./iso";
import { type Palette, readPalette } from "./palette";
import { TALLEST } from "./shapes";

/** Largest tile on screen, and how far below the "whole year" scale the camera can zoom out. */
const MAX_CELL_PX = 280;
const MIN_ZOOM = 0.2;
/** Space around the fitted board (the weekday letters on the left, the months below) and the drag distance of a click. */
const MARGIN = 36;
const CLICK_SLOP = 4;
/** The cached layer is re-rendered this long after the camera rests. */
const SETTLE_MS = 140;
const DOUBLE_CLICK_ZOOM = 2;

export interface EngineEvents {
  /** The pointer is over a day (or left the board: `null`); `at` is the pointer in canvas pixels. */
  onHover(date: string | null, at: Pt | null): void;
  /** A click (not a drag) on a day that can be opened. */
  onActivate(date: string): void;
}

/** A rendered bitmap of the board with its camera and margins (CSS pixels). */
interface Layer {
  canvas: HTMLCanvasElement;
  cam: Camera;
  mx: number;
  my: number;
  version: number;
  dpr: number;
}

export class ForestEngine {
  /** Frames drawn so far: the "no gratuitous motion" tests check that it stands still when nothing happens. */
  frames = 0;
  /** Frames and time of the last pan or zoom gesture, for the debug report. */
  readonly gesture = { frames: 0, start: 0, last: 0 };
  view: View = { cam: { s: 1, tx: 0, ty: 0 }, selected: null, hover: null, grow: null, now: 0 };

  private readonly ctx: CanvasRenderingContext2D | null;
  private board: Board | null = null;
  private labels: Labels = { months: [], weekdays: [] };
  private palette: Palette;
  private w = 1;
  private h = 1;
  private dpr = 1;
  private raf = 0;
  /** Bumped by everything that changes the picture except the camera and the hover. */
  private version = 1;
  private layer: Layer | null = null;
  private settleTimer = 0;
  private settled = false;
  private fitCam: Camera = { s: 1, tx: 0, ty: 0 };
  private atFit = true;
  private pointers = new Map<number, Pt>();
  private drag: { x: number; y: number; cam: Camera; moved: boolean } | null = null;
  private pinch: { dist: number; cam: Camera } | null = null;
  private readonly cleanup: (() => void)[] = [];

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly events: EngineEvents,
  ) {
    this.ctx = canvas.getContext("2d");
    this.palette = readPalette(canvas);
    this.bind();
    if (typeof ResizeObserver !== "undefined") {
      const observer = new ResizeObserver(() => this.resize());
      observer.observe(canvas);
      this.cleanup.push(() => observer.disconnect());
    }
    // Text size and the theme are attributes of <html>: the labels and colours are read again when they change.
    if (typeof MutationObserver !== "undefined") {
      const observer = new MutationObserver(() => this.repaletted());
      observer.observe(document.documentElement, { attributes: true });
      this.cleanup.push(() => observer.disconnect());
    }
    // Fonts arrive after the first layout: the labels are drawn again once they are there.
    void document.fonts?.ready.then(() => this.repaletted()).catch(() => undefined);
    this.resize();
  }

  // ---- what the host calls -----------------------------------------------------------------------------------------

  /**
   * A new table. A new year fits the whole board; the same year keeps the camera, and if today's stage rose the tree grows
   * once (unless motion is off: then it just changes).
   */
  setBoard(board: Board, labels: Labels): void {
    const old = this.board;
    this.board = board;
    this.labels = labels;
    const sameYear = old !== null && old.year === board.year && old.dims.cols === board.dims.cols;
    if (!sameYear) {
      this.view.selected = null;
      this.view.hover = null;
      this.view.grow = null;
      this.fitYear();
    } else {
      this.view.grow = this.growthOf(old, board);
    }
    this.refresh();
  }

  setLabels(labels: Labels): void {
    this.labels = labels;
    this.refresh();
  }

  /** The selected day, or none. `reveal` pans the camera just enough to bring it into view (only for the keyboard). */
  setSelected(date: string | null, reveal = false): void {
    const cell = date !== null ? this.board?.byDate.get(date) : undefined;
    const next = cell?.interactive ? { row: cell.row, col: cell.col } : null;
    if (next?.row === this.view.selected?.row && next?.col === this.view.selected?.col) {
      if (reveal && cell) this.ensureVisible(cell.center);
      return;
    }
    this.view.selected = next;
    if (reveal && cell) this.ensureVisible(cell.center);
    this.refresh();
  }

  /** The "whole year" view. */
  fitYear(): void {
    this.fitCam = this.computeFit();
    this.view.cam = this.fitCam;
    this.atFit = true;
    this.refresh();
  }

  /** Brings some cells (a season) into view, with the same margins as the whole year. */
  fitCells(cells: readonly Cell[]): void {
    const box = cellsBounds(cells);
    if (!box || !this.board) return this.fitYear();
    const s = fitCamera(box, this.w, this.h, MARGIN, TALLEST * TW);
    this.view.cam = this.clamp({ ...s, s: Math.min(MAX_CELL_PX / TW, s.s) });
    this.atFit = false;
    this.refresh();
  }

  dateAt(p: Pt): string | null {
    if (!this.board) return null;
    const c = hitTest(this.view.cam, this.board.dims, p);
    const cell = c && cellAt(this.board, c);
    return cell?.interactive ? (cell.day?.date ?? null) : null;
  }

  /** The screen position of a day's cell centre, or `null` for a date that is not on the board. */
  screenOf(date: string): Pt | null {
    const cell = this.board?.byDate.get(date);
    return cell ? worldToScreen(this.view.cam, cell.center) : null;
  }

  get size(): { w: number; h: number } {
    return { w: this.w, h: this.h };
  }

  destroy(): void {
    cancelAnimationFrame(this.raf);
    window.clearTimeout(this.settleTimer);
    for (const off of this.cleanup) off();
  }

  /** Re-renders the picture (the data changed, the fonts arrived). */
  refresh(): void {
    this.version++;
    this.invalidate();
  }

  // ---- frames ------------------------------------------------------------------------------------------------------

  private invalidate(): void {
    if (this.raf) return;
    this.raf = requestAnimationFrame((t) => {
      this.raf = 0;
      this.frame(t);
    });
  }

  /** One frame: draws, and asks for another one only while the growth animation runs. */
  private frame(now: number): void {
    const ctx = this.ctx;
    const board = this.board;
    if (!ctx || !board) return;
    this.view.now = now;
    this.frames++;
    if (this.gesture.start > 0) this.gesture.frames++;
    this.gesture.last = now;
    if (this.view.grow) this.version++; // the growing tree changes every frame
    const animating = this.drawCached(ctx, board);
    drawHover(ctx, board, this.view, this.palette);
    const grow = this.view.grow;
    if (grow && !animating && now - grow.start >= grow.dur) this.view.grow = null;
    if (animating) this.invalidate();
  }

  /**
   * The board is static between data changes, so a pan or a zoom only moves a bitmap of it: the board is rendered into a
   * layer larger than the window (a quarter of the window of margin on each side) and every gesture frame is one
   * `drawImage`. The layer is re-rendered when the content changes, when the camera has rested for 140 ms (crisp again),
   * or when the view leaves the layer or the scale differs by more than 0.6..1.7 from the layer's. The hover outline is
   * drawn over the layer and does not invalidate it.
   */
  private drawCached(ctx: CanvasRenderingContext2D, board: Board): boolean {
    const layer = this.layer;
    const cam = this.view.cam;
    const same = layer !== null && layer.cam.s === cam.s && layer.cam.tx === cam.tx && layer.cam.ty === cam.ty;
    let animating = false;
    if (!layer || layer.version !== this.version || layer.dpr !== this.dpr || (!same && (this.settled || !this.covers(layer)))) {
      animating = this.renderLayer(board);
      this.settled = false;
    }
    window.clearTimeout(this.settleTimer);
    const current = this.layer;
    if (!current) return animating;
    if (current.cam.s !== cam.s || current.cam.tx !== cam.tx || current.cam.ty !== cam.ty) {
      this.settleTimer = window.setTimeout(() => {
        this.settled = true;
        this.invalidate();
      }, SETTLE_MS);
    }
    this.blit(ctx, current, cam);
    return animating;
  }

  /** Draws the part of the layer that lands in the window: a source rectangle, so the cost follows the window, not the layer. */
  private blit(ctx: CanvasRenderingContext2D, layer: Layer, cam: Camera): void {
    const k = cam.s / layer.cam.s;
    const dx = (-layer.mx - layer.cam.tx) * k + cam.tx;
    const dy = (-layer.my - layer.cam.ty) * k + cam.ty;
    const dw = (layer.canvas.width / layer.dpr) * k;
    const dh = (layer.canvas.height / layer.dpr) * k;
    const x0 = Math.max(0, dx);
    const y0 = Math.max(0, dy);
    const x1 = Math.min(this.w, dx + dw);
    const y1 = Math.min(this.h, dy + dh);
    if (x1 <= x0 || y1 <= y0) return;
    const f = layer.dpr / k;
    ctx.drawImage(layer.canvas, (x0 - dx) * f, (y0 - dy) * f, (x1 - x0) * f, (y1 - y0) * f, x0, y0, x1 - x0, y1 - y0);
  }

  /** Does the layer cover the whole window at the current camera, at a scale close enough to look right? */
  private covers(layer: Layer): boolean {
    const cam = this.view.cam;
    const k = cam.s / layer.cam.s;
    if (k < 0.6 || k > 1.7) return false;
    const x0 = (0 - cam.tx) / k + layer.cam.tx;
    const y0 = (0 - cam.ty) / k + layer.cam.ty;
    const x1 = (this.w - cam.tx) / k + layer.cam.tx;
    const y1 = (this.h - cam.ty) / k + layer.cam.ty;
    return x0 >= -layer.mx && y0 >= -layer.my && x1 <= this.w + layer.mx && y1 <= this.h + layer.my;
  }

  private renderLayer(board: Board): boolean {
    const mx = Math.round(this.w * 0.25);
    const my = Math.round(this.h * 0.25);
    const canvas = this.layer?.canvas ?? document.createElement("canvas");
    canvas.width = Math.max(1, Math.round((this.w + 2 * mx) * this.dpr));
    canvas.height = Math.max(1, Math.round((this.h + 2 * my) * this.dpr));
    const g = canvas.getContext("2d");
    if (!g) return false;
    g.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    const cam = this.view.cam;
    const shifted: View = { ...this.view, cam: { s: cam.s, tx: cam.tx + mx, ty: cam.ty + my } };
    const animating = drawScene(g, this.w + 2 * mx, this.h + 2 * my, board, shifted, this.palette, this.labels);
    this.layer = { canvas, cam, mx, my, version: this.version, dpr: this.dpr };
    return animating;
  }

  // ---- state helpers -----------------------------------------------------------------------------------------------

  /** Today's tree grows once when its stage went up and motion is on; the duration and the curve are design tokens. */
  private growthOf(old: Board, board: Board): Grow | null {
    const before = old.byDate.get(old.today)?.day?.stage ?? 0;
    const after = board.byDate.get(board.today)?.day?.stage ?? 0;
    if (old.today !== board.today || after <= before) return null;
    const style = getComputedStyle(document.documentElement);
    const dur = parseDuration(style.getPropertyValue("--motion-duration-grow"));
    if (dur <= 0) return null;
    return { date: board.today, from: before, to: after, start: performance.now(), dur, ease: cubicBezier(style.getPropertyValue("--motion-easing-enter")) };
  }

  private repaletted(): void {
    this.palette = readPalette(this.canvas);
    this.refresh();
  }

  private resize(): void {
    const r = this.canvas.getBoundingClientRect();
    this.dpr = window.devicePixelRatio || 1;
    this.w = Math.max(1, Math.round(r.width));
    this.h = Math.max(1, Math.round(r.height));
    this.canvas.width = Math.round(this.w * this.dpr);
    this.canvas.height = Math.round(this.h * this.dpr);
    this.ctx?.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    this.fitCam = this.computeFit();
    if (this.atFit) this.view.cam = this.fitCam;
    this.refresh();
  }

  private computeFit(): Camera {
    if (!this.board) return { s: 1, tx: 0, ty: 0 };
    // Room above the board for the tallest tree.
    return fitCamera(boardBounds(this.board.dims), this.w, this.h, MARGIN, TALLEST * TW);
  }

  private clamp(cam: Camera): Camera {
    return this.board ? clampCamera(cam, boardBounds(this.board.dims), this.w, this.h, 120) : cam;
  }

  private minScale(): number {
    return this.fitCam.s * MIN_ZOOM;
  }

  private zoomBy(p: Pt, factor: number): void {
    this.view.cam = this.clamp(zoomAt(this.view.cam, p, factor, this.minScale(), MAX_CELL_PX / TW));
    this.atFit = false;
    this.touchGesture();
    this.hoverFrom(p);
    this.invalidate();
  }

  /** Pan just enough to bring a world point into view, with room above it for the tree. */
  private ensureVisible(world: Pt): void {
    const p = worldToScreen(this.view.cam, world);
    const m = 60;
    const tree = TW * this.view.cam.s;
    let { tx, ty } = this.view.cam;
    if (p.x < m) tx += m - p.x;
    if (p.x > this.w - m) tx -= p.x - (this.w - m);
    if (p.y < m + tree) ty += m + tree - p.y;
    if (p.y > this.h - m) ty -= p.y - (this.h - m);
    if (tx !== this.view.cam.tx || ty !== this.view.cam.ty) {
      this.view.cam = this.clamp({ ...this.view.cam, tx, ty });
      this.atFit = false;
      this.invalidate();
    }
  }

  private touchGesture(): void {
    const now = performance.now();
    if (this.gesture.start === 0 || now - this.gesture.last > 250) {
      this.reportGesture();
      this.gesture.start = now;
      this.gesture.frames = 0;
    }
    this.gesture.last = now;
  }

  /** The frame rate of the finished gesture, numbers only (the real-hardware check of the slice reads the web console). */
  private reportGesture(): void {
    const { frames, start, last } = this.gesture;
    if (start > 0 && frames > 10 && last > start) console.debug(`forest gesture: ${frames} frames, ${Math.round((frames * 1000) / (last - start))} fps`);
  }

  // ---- input -------------------------------------------------------------------------------------------------------

  private local(e: { clientX: number; clientY: number }): Pt {
    const r = this.canvas.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  private hoverFrom(p: Pt): void {
    const date = this.dateAt(p);
    const cell = date !== null ? this.board?.byDate.get(date) : undefined;
    this.setHover(cell ? { row: cell.row, col: cell.col } : null, date, p);
  }

  private setHover(cell: Cell | null, date: string | null, p: Pt | null): void {
    const same = cell?.row === this.view.hover?.row && cell?.col === this.view.hover?.col;
    this.view.hover = cell;
    if (!same) this.invalidate();
    // The card follows the pointer, so the host hears every move over a day, not only changes of the cell.
    this.events.onHover(date, cell ? p : null);
  }

  private bind(): void {
    const c = this.canvas;
    const on = <K extends keyof HTMLElementEventMap>(type: K, handler: (e: HTMLElementEventMap[K]) => void, options?: AddEventListenerOptions) => {
      c.addEventListener(type, handler, options);
      this.cleanup.push(() => c.removeEventListener(type, handler));
    };
    on("pointerdown", (e) => {
      c.setPointerCapture?.(e.pointerId);
      const p = this.local(e);
      this.pointers.set(e.pointerId, p);
      if (this.pointers.size === 2) {
        const [a, b] = [...this.pointers.values()];
        if (a && b) this.pinch = { dist: Math.hypot(a.x - b.x, a.y - b.y), cam: this.view.cam };
        this.drag = null;
      } else {
        this.drag = { x: p.x, y: p.y, cam: this.view.cam, moved: false };
      }
    });
    on("pointermove", (e) => {
      const p = this.local(e);
      if (this.pointers.has(e.pointerId)) this.pointers.set(e.pointerId, p);
      if (this.pinch && this.pointers.size === 2) {
        const [a, b] = [...this.pointers.values()];
        if (a && b) {
          const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
          const k = Math.hypot(a.x - b.x, a.y - b.y) / this.pinch.dist;
          this.view.cam = this.clamp(zoomAt(this.pinch.cam, mid, k, this.minScale(), MAX_CELL_PX / TW));
          this.atFit = false;
          this.touchGesture();
          this.invalidate();
        }
        return;
      }
      if (this.drag) {
        const dx = p.x - this.drag.x;
        const dy = p.y - this.drag.y;
        if (!this.drag.moved && Math.hypot(dx, dy) > CLICK_SLOP) {
          this.drag.moved = true;
          c.dataset.dragging = "true";
          this.setHover(null, null, null);
        }
        if (this.drag.moved) {
          this.view.cam = this.clamp({ ...this.drag.cam, tx: this.drag.cam.tx + dx, ty: this.drag.cam.ty + dy });
          this.atFit = false;
          this.touchGesture();
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
        const date = this.dateAt(p);
        if (date) this.events.onActivate(date);
      }
      this.drag = null;
      delete c.dataset.dragging;
    };
    on("pointerup", end);
    on("pointercancel", end);
    on("pointerleave", () => {
      if (!this.drag?.moved) this.setHover(null, null, null);
    });
    on(
      "wheel",
      (e) => {
        e.preventDefault();
        const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1;
        const k = e.ctrlKey ? 0.01 : 0.0018; // ctrl + wheel is how Chromium reports a trackpad pinch
        this.zoomBy(this.local(e), Math.exp(-e.deltaY * unit * k));
      },
      { passive: false },
    );
    on("dblclick", (e) => this.zoomBy(this.local(e), DOUBLE_CLICK_ZOOM));
    // Safari and WKWebView report a trackpad pinch as gesture events (not in the DOM typings).
    let gestureStart = 1;
    const gesture = c as unknown as EventTarget;
    const onGestureStart = (e: Event) => {
      e.preventDefault();
      gestureStart = 1;
    };
    const onGestureChange = (e: Event) => {
      e.preventDefault();
      const ge = e as unknown as { scale: number; clientX: number; clientY: number };
      this.zoomBy(this.local(ge), ge.scale / gestureStart);
      gestureStart = ge.scale;
    };
    gesture.addEventListener("gesturestart", onGestureStart);
    gesture.addEventListener("gesturechange", onGestureChange);
    this.cleanup.push(() => {
      gesture.removeEventListener("gesturestart", onGestureStart);
      gesture.removeEventListener("gesturechange", onGestureChange);
    });
  }
}
