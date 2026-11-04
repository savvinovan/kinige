import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type RefObject } from "react";
import {
  DOUBLE_ZOOM,
  MAX_ZOOM,
  MIN_ZOOM,
  ORIGIN,
  clampPan,
  clampZoom,
  fitScale,
  fittedSize,
  stepZoom,
  zoomAround,
  type Point,
  type Size,
} from "./viewerMath";

/** Zoom, pan and turn of the photo on screen (KINIG-87). View only: nothing here is stored, and a new photo starts from fit. */
interface ViewerState {
  zoom: number;
  pan: Point;
  /** Counter-clockwise quarter turns, counted up (not modulo 4) so that a transition always turns the short way. */
  turns: number;
  /** The last change was a button, key or double click: it eases in. Wheel and drag follow the input directly. */
  animate: boolean;
}

export interface ViewerTransform {
  /** What is on screen now (the latest committed state, also between renders). */
  current: () => Pick<ViewerState, "zoom" | "pan" | "turns">;
  /** Style of the `<img>`: the box that fits the stage, and the transform for pan, zoom and turn. */
  imageStyle: CSSProperties | undefined;
  animate: boolean;
  zoomed: boolean;
  /** Zoom as a whole percentage of fit: fit is 100. */
  percent: number;
  atMin: boolean;
  atMax: boolean;
  zoomTo: (zoom: number, anchor?: Point, animate?: boolean) => void;
  zoomBy: (factor: number, anchor: Point) => void;
  zoomStep: (direction: -1 | 1) => void;
  resetZoom: () => void;
  /** Fit and 2× at the anchor: a double click. */
  toggleZoom: (anchor: Point) => void;
  rotate: () => void;
  panTo: (pan: Point) => void;
}

interface Options {
  /** Changes with the photo: zoom, pan and turn start over. */
  resetKey: string;
  /** The size of the photo as drawn (before turning); `null` until known, then CSS fits it. */
  natural: Size | null;
  /** The area the photo is fitted into: its size is the stage, its centre the origin of the pan. */
  figure: RefObject<HTMLElement | null>;
  /** There is a photo to move: with a "No preview" tile the commands do nothing. */
  enabled: boolean;
}

const START: ViewerState = { zoom: MIN_ZOOM, pan: ORIGIN, turns: 0, animate: false };
const NO_STAGE: Size = { width: 0, height: 0 };
const DEGREES_PER_TURN = 90;

export function useViewerTransform({ resetKey, natural, figure, enabled }: Options): ViewerTransform {
  const [state, setState] = useState(START);
  const [seenKey, setSeenKey] = useState(resetKey);
  const [stage, setStage] = useState(NO_STAGE);
  // The handlers (wheel, keys, pointer) are bound once and read the newest state here; a commit also updates it at once, so two wheel
  // events in one frame add up.
  const latest = useRef({ state, stage, natural, enabled });
  useEffect(() => {
    latest.current = { state, stage, natural, enabled };
  });

  // A new photo: back to fit and upright, without easing (adjusting state while rendering, React's way to reset on a changed key).
  if (seenKey !== resetKey) {
    setSeenKey(resetKey);
    setState(START);
  }

  useLayoutEffect(() => {
    const element = figure.current;
    if (!element) return;
    const measure = () => {
      const box = element.getBoundingClientRect();
      setStage((was) => (was.width === box.width && was.height === box.height ? was : { width: box.width, height: box.height }));
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [figure]);

  const commit = (next: ViewerState) => {
    latest.current = { ...latest.current, state: next };
    setState(next);
  };

  const zoomTo = (zoom: number, anchor: Point = ORIGIN, animate = false) => {
    const { state: was, stage: area, natural: size, enabled: on } = latest.current;
    if (!on) return;
    const next = zoomAround(was, zoom, anchor, fittedSize(size, area, was.turns), area);
    commit({ ...was, ...next, animate });
  };

  const resetZoom = () => {
    const { state: was, enabled: on } = latest.current;
    if (on) commit({ ...was, zoom: MIN_ZOOM, pan: ORIGIN, animate: true });
  };

  const rotate = () => {
    const { state: was, stage: area, natural: size, enabled: on } = latest.current;
    if (!on) return;
    const turns = was.turns + 1;
    commit({ ...was, turns, pan: clampPan(was.pan, was.zoom, fittedSize(size, area, turns), area), animate: true });
  };

  const panTo = (pan: Point) => {
    const { state: was, stage: area, natural: size, enabled: on } = latest.current;
    if (on) commit({ ...was, pan: clampPan(pan, was.zoom, fittedSize(size, area, was.turns), area), animate: false });
  };

  const zoomed = state.zoom > MIN_ZOOM;
  // The pan is clamped again on render: the window may have been resized or the photo turned since it was set.
  const fitted = fittedSize(natural, stage, state.turns);
  const pan = clampPan(state.pan, state.zoom, fitted, stage);

  return {
    current: () => latest.current.state,
    imageStyle: imageStyle(natural, stage, state, pan),
    animate: state.animate,
    zoomed,
    percent: Math.round(state.zoom * 100),
    atMin: !zoomed,
    atMax: state.zoom >= MAX_ZOOM,
    zoomTo,
    zoomBy: (factor, anchor) => zoomTo(latest.current.state.zoom * factor, anchor, false),
    zoomStep: (direction) => zoomTo(stepZoom(latest.current.state.zoom, direction), ORIGIN, true),
    resetZoom,
    toggleZoom: (anchor) => (latest.current.state.zoom > MIN_ZOOM ? resetZoom() : zoomTo(DOUBLE_ZOOM, anchor, true)),
    rotate,
    panTo,
  };
}

/**
 * The photo is laid out at the size that fits the stage upright; turning and zoom are only the transform. The scale is
 * `zoom × (fit of the turned photo ÷ fit of the upright one)`, so a turned landscape photo takes the stage height at zoom 1.
 * Until the stage and the photo are measured, CSS (`max-width`, `max-height`) fits it and only the transform is set.
 */
function imageStyle(natural: Size | null, stage: Size, state: ViewerState, pan: Point): CSSProperties | undefined {
  const upright = fitScale(natural, stage, 0);
  const turned = fitScale(natural, stage, state.turns);
  const ratio = upright !== null && turned !== null ? turned / upright : 1;
  const box = natural && upright !== null ? { width: natural.width * upright, height: natural.height * upright } : undefined;
  if (!box && state.zoom === MIN_ZOOM && state.turns === 0) return undefined;
  const scale = clampZoom(state.zoom) * ratio;
  return {
    ...box,
    transform: `translate(${pan.x}px, ${pan.y}px) scale(${scale}) rotate(${-DEGREES_PER_TURN * state.turns}deg)`,
  };
}
