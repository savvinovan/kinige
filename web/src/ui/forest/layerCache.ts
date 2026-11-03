import type { Camera } from "./iso";

/** The cached layer is re-rendered this long after the camera rests. */
const SETTLE_MS = 140;

/** A rendered bitmap of the board with its camera and margins (CSS pixels). */
interface Layer {
  canvas: HTMLCanvasElement;
  cam: Camera;
  mx: number;
  my: number;
  version: number;
  dpr: number;
}

/** Draws the picture of the board into a layer: the size in CSS pixels and the camera already shifted by the layer's margin. */
export type Paint = (g: CanvasRenderingContext2D, w: number, h: number, cam: Camera) => boolean;

/**
 * The board is static between data changes, so a pan or a zoom only moves a bitmap of it: the board is rendered into a layer
 * larger than the window (a quarter of the window of margin on each side) and every gesture frame is one `drawImage` of the
 * visible part. The layer is re-rendered when the content changes (`version`), when the camera has rested for 140 ms (crisp
 * again), or when the view leaves the layer or the scale differs by more than 0.6..1.7 from the layer's. The hover outline is
 * drawn over the layer by the caller and does not invalidate it. (Measured in spike S3: this lifts Linux WebKit at 1x from
 * 36..38 to 61 FPS.)
 */
export class LayerCache {
  private layer: Layer | null = null;
  private settled = false;
  private timer = 0;

  /** Draws one frame into `ctx`; the answer is `true` while the painted picture is animating. */
  draw(ctx: CanvasRenderingContext2D, size: { w: number; h: number; dpr: number }, cam: Camera, version: number, paint: Paint, onSettled: () => void): boolean {
    const layer = this.layer;
    const same = layer !== null && layer.cam.s === cam.s && layer.cam.tx === cam.tx && layer.cam.ty === cam.ty;
    let animating = false;
    if (!layer || layer.version !== version || layer.dpr !== size.dpr || (!same && (this.settled || !this.covers(layer, size, cam)))) {
      animating = this.render(size, cam, version, paint);
      this.settled = false;
    }
    window.clearTimeout(this.timer);
    const current = this.layer;
    if (!current) return animating;
    if (current.cam.s !== cam.s || current.cam.tx !== cam.tx || current.cam.ty !== cam.ty) {
      this.timer = window.setTimeout(() => {
        this.settled = true;
        onSettled();
      }, SETTLE_MS);
    }
    this.blit(ctx, current, size, cam);
    return animating;
  }

  dispose(): void {
    window.clearTimeout(this.timer);
  }

  /** Draws the part of the layer that lands in the window: a source rectangle, so the cost follows the window, not the layer. */
  private blit(ctx: CanvasRenderingContext2D, layer: Layer, size: { w: number; h: number }, cam: Camera): void {
    const k = cam.s / layer.cam.s;
    const dx = (-layer.mx - layer.cam.tx) * k + cam.tx;
    const dy = (-layer.my - layer.cam.ty) * k + cam.ty;
    const dw = (layer.canvas.width / layer.dpr) * k;
    const dh = (layer.canvas.height / layer.dpr) * k;
    const x0 = Math.max(0, dx);
    const y0 = Math.max(0, dy);
    const x1 = Math.min(size.w, dx + dw);
    const y1 = Math.min(size.h, dy + dh);
    if (x1 <= x0 || y1 <= y0) return;
    const f = layer.dpr / k;
    ctx.drawImage(layer.canvas, (x0 - dx) * f, (y0 - dy) * f, (x1 - x0) * f, (y1 - y0) * f, x0, y0, x1 - x0, y1 - y0);
  }

  /** Does the layer cover the whole window at the current camera, at a scale close enough to look right? */
  private covers(layer: Layer, size: { w: number; h: number }, cam: Camera): boolean {
    const k = cam.s / layer.cam.s;
    if (k < 0.6 || k > 1.7) return false;
    const x0 = (0 - cam.tx) / k + layer.cam.tx;
    const y0 = (0 - cam.ty) / k + layer.cam.ty;
    const x1 = (size.w - cam.tx) / k + layer.cam.tx;
    const y1 = (size.h - cam.ty) / k + layer.cam.ty;
    return x0 >= -layer.mx && y0 >= -layer.my && x1 <= size.w + layer.mx && y1 <= size.h + layer.my;
  }

  private render(size: { w: number; h: number; dpr: number }, cam: Camera, version: number, paint: Paint): boolean {
    const mx = Math.round(size.w * 0.25);
    const my = Math.round(size.h * 0.25);
    const canvas = this.layer?.canvas ?? document.createElement("canvas");
    canvas.width = Math.max(1, Math.round((size.w + 2 * mx) * size.dpr));
    canvas.height = Math.max(1, Math.round((size.h + 2 * my) * size.dpr));
    const g = canvas.getContext("2d");
    if (!g) return false;
    g.setTransform(size.dpr, 0, 0, size.dpr, 0, 0);
    const animating = paint(g, size.w + 2 * mx, size.h + 2 * my, { s: cam.s, tx: cam.tx + mx, ty: cam.ty + my });
    this.layer = { canvas, cam, mx, my, version, dpr: size.dpr };
    return animating;
  }
}
