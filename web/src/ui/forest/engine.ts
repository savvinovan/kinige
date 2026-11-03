// The board engine: camera, the growth animation and the frame loop around the drawing (draw.ts), the layer cache
// (layerCache.ts) and the pointer input (gestures.ts). Frames are requested on demand: when nothing changes and nothing
// animates, no frame is drawn.

import { type Board, cellAt } from "./board";
import { type Grow, type Labels, type View, drawHover, drawScene } from "./draw";
import { cubicBezier, parseDuration } from "./easing";
import { bindGestures } from "./gestures";
import { type Camera, type Cell, type Pt, TW, boardBounds, cellsBounds, clampCamera, fitCamera, hitTest, worldToScreen, zoomAt } from "./iso";
import { LayerCache } from "./layerCache";
import { type Palette, readPalette } from "./palette";
import { TALLEST } from "./shapes";

/** Largest tile on screen, and how far below the "whole year" scale the camera can zoom out. */
const MAX_CELL_PX = 280;
const MIN_ZOOM = 0.2;
/** Space around the fitted board (the weekday letters on the left, the months below), and how much of it a drag cannot lose. */
const MARGIN = 36;
const KEEP_ON_SCREEN = 120;
/** A pause longer than this ends a gesture for the frame-rate report. */
const GESTURE_GAP_MS = 250;

export interface EngineEvents {
  /** The pointer is over a day (or left the board: `null`); `at` is the pointer in canvas pixels. */
  onHover(date: string | null, at: Pt | null): void;
  /** A click (not a drag) on a day that can be opened. */
  onActivate(date: string): void;
}

export class ForestEngine {
  /** Frames drawn so far: the "no gratuitous motion" tests check that it stands still when nothing happens. */
  frames = 0;
  /** Frames and times of the current pan or zoom gesture, for the frame-rate report. */
  private gesture = { frames: 0, start: 0, last: 0 };
  view: View = { cam: { s: 1, tx: 0, ty: 0 }, selected: null, hover: null, grow: null, now: 0 };

  private readonly ctx: CanvasRenderingContext2D | null;
  private readonly layer = new LayerCache();
  private board: Board | null = null;
  private labels: Labels = { months: [], monthsNarrow: [], weekdays: [] };
  private palette: Palette;
  private w = 1;
  private h = 1;
  private dpr = 1;
  private raf = 0;
  /** Bumped by everything that changes the picture except the camera and the hover. */
  private version = 1;
  private fitCam: Camera = { s: 1, tx: 0, ty: 0 };
  private atFit = true;
  private readonly cleanup: (() => void)[] = [];

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly events: EngineEvents,
  ) {
    this.ctx = canvas.getContext("2d");
    this.palette = readPalette(canvas);
    this.cleanup.push(
      bindGestures(canvas, {
        camera: () => this.view.cam,
        setCamera: (cam) => this.moveTo(cam),
        zoomBy: (at, factor) => this.zoomBy(at, factor),
        scaleRange: () => ({ min: this.minScale(), max: MAX_CELL_PX / TW }),
        hover: (point) => (point ? this.hoverFrom(point) : this.setHover(null, null, null)),
        click: (point) => {
          const date = this.dateAt(point);
          if (date) this.events.onActivate(date);
        },
        moved: () => this.touchGesture(),
      }),
    );
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

  /** The selected day, or none. `reveal` pans the camera just enough to bring it into view (only for the keyboard). */
  setSelected(date: string | null, reveal = false): void {
    const cell = date !== null ? this.board?.byDate.get(date) : undefined;
    const next = cell?.interactive ? { row: cell.row, col: cell.col } : null;
    const changed = next?.row !== this.view.selected?.row || next?.col !== this.view.selected?.col;
    this.view.selected = next;
    if (reveal && cell) this.ensureVisible(cell.center);
    if (changed) this.refresh();
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
    const fit = fitCamera(box, this.w, this.h, MARGIN, TALLEST * TW);
    this.view.cam = this.clamp({ ...fit, s: Math.min(MAX_CELL_PX / TW, fit.s) });
    this.atFit = false;
    this.invalidate();
  }

  dateAt(p: Pt): string | null {
    if (!this.board) return null;
    const c = hitTest(this.view.cam, this.board.dims, p);
    const cell = c && cellAt(this.board, c);
    return cell?.interactive ? (cell.day?.date ?? null) : null;
  }

  /** The screen position of a day's cell centre in canvas pixels, or `null` for a date that is not on the board (the click-through test). */
  screenOf(date: string): Pt | null {
    const cell = this.board?.byDate.get(date);
    return cell ? worldToScreen(this.view.cam, cell.center) : null;
  }

  get size(): { w: number; h: number } {
    return { w: this.w, h: this.h };
  }

  destroy(): void {
    cancelAnimationFrame(this.raf);
    this.layer.dispose();
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
    this.gesture.frames++;
    if (this.view.grow) this.version++; // the growing tree changes every frame
    const animating = this.layer.draw(
      ctx,
      { w: this.w, h: this.h, dpr: this.dpr },
      this.view.cam,
      this.version,
      (g, w, h, cam) => drawScene(g, w, h, board, { ...this.view, cam }, this.palette, this.labels),
      () => this.invalidate(),
    );
    drawHover(ctx, board, this.view, this.palette);
    const grow = this.view.grow;
    if (grow && !animating && now - grow.start >= grow.dur) this.view.grow = null;
    if (animating) this.invalidate();
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
    return this.board ? clampCamera(cam, boardBounds(this.board.dims), this.w, this.h, KEEP_ON_SCREEN) : cam;
  }

  private minScale(): number {
    return this.fitCam.s * MIN_ZOOM;
  }

  /** A gesture moved the camera. */
  private moveTo(cam: Camera): void {
    this.view.cam = this.clamp(cam);
    this.atFit = false;
    this.invalidate();
  }

  private zoomBy(p: Pt, factor: number): void {
    this.moveTo(zoomAt(this.view.cam, p, factor, this.minScale(), MAX_CELL_PX / TW));
    this.touchGesture();
    this.hoverFrom(p);
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
    if (tx !== this.view.cam.tx || ty !== this.view.cam.ty) this.moveTo({ ...this.view.cam, tx, ty });
  }

  private touchGesture(): void {
    const now = performance.now();
    if (this.gesture.start === 0 || now - this.gesture.last > GESTURE_GAP_MS) {
      this.reportGesture();
      this.gesture = { frames: 0, start: now, last: now };
    }
    this.gesture.last = now;
  }

  /** The frame rate of the finished gesture, numbers only (a dev build prints it to the web console; a release build has no way to read it). */
  private reportGesture(): void {
    const { frames, start, last } = this.gesture;
    if (start > 0 && frames > 10 && last > start) console.debug(`forest gesture: ${frames} frames, ${Math.round((frames * 1000) / (last - start))} fps`);
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
}
