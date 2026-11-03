import { type Camera, type Pt, zoomAt } from "./iso";

/** The drag distance below which a press and release is a click. */
const CLICK_SLOP = 4;
const DOUBLE_CLICK_ZOOM = 2;

/** What the gestures need from the board and what they tell it. */
export interface GestureHost {
  camera(): Camera;
  /** Sets a camera from a gesture (the board clamps it and keeps it out of the "fits the pane" state). */
  setCamera(cam: Camera): void;
  /** Zoom factor about a point; the board clamps the scale. */
  zoomBy(at: Pt, factor: number): void;
  /** The scale limits of the current board, for the pinch. */
  scaleRange(): { min: number; max: number };
  /** The pointer is over the board (`point`), or has left it or started to drag (`null`). */
  hover(point: Pt | null): void;
  /** A click that was not a drag. */
  click(point: Pt): void;
  /** A gesture changed the camera (frame-rate bookkeeping). */
  moved(): void;
}

/**
 * Pointer input of the board: drag pans, wheel zooms about the cursor, `ctrl` + wheel is a trackpad pinch in Chromium and
 * WebView2, `gesturechange` is one in Safari and WKWebView, two pointers pinch, a double click zooms in. Returns the function
 * that removes every listener. The Safari gesture events are written from the API and were never run (ADR-0013, not verified).
 */
export function bindGestures(canvas: HTMLCanvasElement, host: GestureHost): () => void {
  const pointers = new Map<number, Pt>();
  let drag: { x: number; y: number; cam: Camera; moved: boolean } | null = null;
  let pinch: { dist: number; cam: Camera } | null = null;
  const offs: (() => void)[] = [];

  const local = (e: { clientX: number; clientY: number }): Pt => {
    const r = canvas.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const on = <K extends keyof HTMLElementEventMap>(type: K, handler: (e: HTMLElementEventMap[K]) => void, options?: AddEventListenerOptions) => {
    canvas.addEventListener(type, handler, options);
    offs.push(() => canvas.removeEventListener(type, handler));
  };

  // A click that no pointer press came before: a press made by assistive technology (an accessibility "press" action
  // dispatches a simulated click, not pointer events), or by the screen scenarios of the slice. A real click is a press,
  // a release and a click, and the release has already handled it.
  let pressed = false;
  on("click", (e) => {
    if (pressed) pressed = false;
    else host.click(local(e));
  });

  on("pointerdown", (e) => {
    pressed = true;
    canvas.setPointerCapture?.(e.pointerId);
    const p = local(e);
    pointers.set(e.pointerId, p);
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      if (a && b) pinch = { dist: Math.hypot(a.x - b.x, a.y - b.y), cam: host.camera() };
      drag = null;
    } else {
      drag = { x: p.x, y: p.y, cam: host.camera(), moved: false };
    }
  });

  on("pointermove", (e) => {
    const p = local(e);
    if (pointers.has(e.pointerId)) pointers.set(e.pointerId, p);
    if (pinch && pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      if (a && b) {
        const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
        const { min, max } = host.scaleRange();
        host.setCamera(zoomAt(pinch.cam, mid, Math.hypot(a.x - b.x, a.y - b.y) / pinch.dist, min, max));
        host.moved();
      }
      return;
    }
    if (drag) {
      const dx = p.x - drag.x;
      const dy = p.y - drag.y;
      if (!drag.moved && Math.hypot(dx, dy) > CLICK_SLOP) {
        drag.moved = true;
        canvas.dataset.dragging = "true";
        host.hover(null);
      }
      if (drag.moved) {
        host.setCamera({ ...drag.cam, tx: drag.cam.tx + dx, ty: drag.cam.ty + dy });
        host.moved();
      }
      return;
    }
    host.hover(p);
  });

  const end = (e: PointerEvent) => {
    pointers.delete(e.pointerId);
    if (pointers.size < 2) pinch = null;
    // A cancelled press (the system took the touch) has no click after it.
    if (e.type === "pointercancel") pressed = false;
    if (drag && !drag.moved && e.type === "pointerup") host.click(local(e));
    drag = null;
    delete canvas.dataset.dragging;
  };
  on("pointerup", end);
  on("pointercancel", end);
  on("pointerleave", () => {
    if (!drag?.moved) host.hover(null);
  });

  on(
    "wheel",
    (e) => {
      e.preventDefault();
      const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1;
      const k = e.ctrlKey ? 0.01 : 0.0018; // ctrl + wheel is how Chromium reports a trackpad pinch
      host.zoomBy(local(e), Math.exp(-e.deltaY * unit * k));
    },
    { passive: false },
  );
  on("dblclick", (e) => host.zoomBy(local(e), DOUBLE_CLICK_ZOOM));

  // Safari and WKWebView report a trackpad pinch as gesture events (not in the DOM typings).
  let gestureStart = 1;
  const target = canvas as unknown as EventTarget;
  const onGestureStart = (e: Event) => {
    e.preventDefault();
    gestureStart = 1;
  };
  const onGestureChange = (e: Event) => {
    e.preventDefault();
    const ge = e as unknown as { scale: number; clientX: number; clientY: number };
    host.zoomBy(local(ge), ge.scale / gestureStart);
    gestureStart = ge.scale;
  };
  target.addEventListener("gesturestart", onGestureStart);
  target.addEventListener("gesturechange", onGestureChange);
  offs.push(() => {
    target.removeEventListener("gesturestart", onGestureStart);
    target.removeEventListener("gesturechange", onGestureChange);
  });

  return () => {
    for (const off of offs) off();
  };
}
