import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type RefObject } from "react";
import type { ViewerTransform } from "./useViewerTransform";
import { anchorOf, wheelZoomFactor } from "./viewerMath";

/** A press that moves less than this (px) is a click, not a drag. */
export const CLICK_SLOP_PX = 4;
/** Two clicks on the photo within this (ms) are a double click; a single click on the photo at fit size closes the viewer after it. */
export const DOUBLE_CLICK_MS = 250;

type Where = "image" | "backdrop";

interface Press {
  id: number;
  x: number;
  y: number;
  where: Where;
  moved: boolean;
  /** The pan when the press began. */
  from: { x: number; y: number };
}

interface Tap {
  at: number;
  x: number;
  y: number;
}

/** The pinch of WebKit (Safari, WKWebView): `scale` is relative to the start of the gesture. Not in the DOM typings. */
interface GestureLike extends Event {
  scale: number;
  clientX?: number;
  clientY?: number;
}

/**
 * The pointer, wheel and keys of the viewer (KINIG-87), on top of `useViewerTransform`.
 *
 * Closing and dragging are decided from pointer events on the dialog root: the stage covers the scrim, so a click handler on the scrim alone
 * never fires. A press that starts on a button, the counter or the toolbar (`button`, `[data-viewer-control]`) is ignored; one that starts
 * on the photo (`[data-viewer-image]`) or anywhere else (the backdrop) and moves less than `CLICK_SLOP_PX` is a click: the backdrop closes,
 * the photo closes only at fit size, a second click on it within `DOUBLE_CLICK_MS` toggles the zoom instead. A drag pans a zoomed photo.
 * Cmd or Ctrl with the wheel (and the WebKit pinch) zooms at the cursor, a plain wheel does nothing. Keys: `+`/`=`, `-`, `0`, `R`.
 */
export function useViewerInput(
  root: RefObject<HTMLElement | null>,
  figure: RefObject<HTMLElement | null>,
  viewer: ViewerTransform,
  resetKey: string,
  onClose: () => void,
) {
  const [dragging, setDragging] = useState(false);
  const live = useRef({ viewer, onClose });
  useEffect(() => {
    live.current = { viewer, onClose };
  });
  const press = useRef<Press | null>(null);
  const lastTap = useRef<Tap | null>(null);
  const closeTimer = useRef<number | undefined>(undefined);

  const cancelClose = () => {
    window.clearTimeout(closeTimer.current);
    closeTimer.current = undefined;
  };
  const anchorAt = (clientX: number, clientY: number) => anchorIn(figure, clientX, clientY);

  // Another photo or no viewer: a pending close or a half double click belongs to the old one.
  useEffect(() => {
    lastTap.current = null;
    return () => window.clearTimeout(closeTimer.current);
  }, [resetKey]);

  useEffect(() => {
    const dialog = root.current;
    if (!dialog) return;

    // Not passive: `preventDefault` stops the page zoom that Ctrl + wheel (a Chromium pinch) would start.
    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey && !event.metaKey) return;
      event.preventDefault();
      live.current.viewer.zoomBy(wheelZoomFactor(event.deltaY, event.deltaMode), anchorIn(figure, event.clientX, event.clientY));
    };
    let gestureStart = 1;
    const onGestureStart = (event: Event) => {
      event.preventDefault();
      gestureStart = live.current.viewer.current().zoom;
    };
    const onGestureChange = (event: Event) => {
      event.preventDefault();
      const gesture = event as GestureLike;
      live.current.viewer.zoomTo(gestureStart * gesture.scale, anchorIn(figure, gesture.clientX ?? NaN, gesture.clientY ?? NaN));
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) return;
      const { viewer: view } = live.current;
      const { key, code } = event;
      if (key === "+" || key === "=" || code === "NumpadAdd") view.zoomStep(1);
      else if (key === "-" || code === "NumpadSubtract") view.zoomStep(-1);
      else if (key === "0" || code === "Digit0" || code === "Numpad0") view.resetZoom();
      // `code` as well as `key`: on the Russian layout the R key types "к".
      else if (key === "r" || key === "R" || code === "KeyR") view.rotate();
      else return;
      event.preventDefault();
    };

    dialog.addEventListener("wheel", onWheel, { passive: false });
    dialog.addEventListener("gesturestart", onGestureStart);
    dialog.addEventListener("gesturechange", onGestureChange);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      dialog.removeEventListener("wheel", onWheel);
      dialog.removeEventListener("gesturestart", onGestureStart);
      dialog.removeEventListener("gesturechange", onGestureChange);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [root, figure]);

  const click = (where: Where, x: number, y: number) => {
    const { viewer: view, onClose: close } = live.current;
    if (where === "backdrop") {
      cancelClose();
      lastTap.current = null;
      close();
      return;
    }
    const now = Date.now();
    const before = lastTap.current;
    if (before && now - before.at < DOUBLE_CLICK_MS && Math.hypot(x - before.x, y - before.y) < CLICK_SLOP_PX * 2) {
      cancelClose();
      lastTap.current = null;
      view.toggleZoom(anchorAt(x, y));
      return;
    }
    lastTap.current = { at: now, x, y };
    // At fit size a click on the photo closes, but a second click may still turn it into a double click: wait for it.
    if (!view.zoomed) closeTimer.current = window.setTimeout(() => live.current.onClose(), DOUBLE_CLICK_MS);
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLElement>) => {
    if (press.current || event.button !== 0) return;
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest("button, [data-viewer-control]")) return;
    const where = target?.closest("[data-viewer-image]") ? "image" : "backdrop";
    press.current = { id: event.pointerId, x: event.clientX, y: event.clientY, where, moved: false, from: live.current.viewer.current().pan };
    try {
      event.currentTarget.setPointerCapture(event.pointerId);
    } catch {
      // No pointer capture (a test DOM): the moves still reach the dialog while the pointer is over it.
    }
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLElement>) => {
    const current = press.current;
    if (!current || current.id !== event.pointerId) return;
    const dx = event.clientX - current.x;
    const dy = event.clientY - current.y;
    if (!current.moved) {
      if (Math.hypot(dx, dy) < CLICK_SLOP_PX) return;
      current.moved = true;
      cancelClose();
      lastTap.current = null;
    }
    const { viewer: view } = live.current;
    if (!view.zoomed) return;
    setDragging(true);
    view.panTo({ x: current.from.x + dx, y: current.from.y + dy });
  };

  const onPointerUp = (event: ReactPointerEvent<HTMLElement>) => {
    const current = press.current;
    if (!current || current.id !== event.pointerId) return;
    press.current = null;
    setDragging(false);
    if (!current.moved) click(current.where, event.clientX, event.clientY);
  };

  const onPointerCancel = () => {
    press.current = null;
    setDragging(false);
  };

  return { dragging, handlers: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel } };
}

/** The pointer relative to the centre of the stage; an unmounted stage counts as an empty box at the origin. */
function anchorIn(figure: RefObject<HTMLElement | null>, clientX: number, clientY: number) {
  return anchorOf(clientX, clientY, figure.current?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 });
}
