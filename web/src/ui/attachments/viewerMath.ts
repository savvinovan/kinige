// The maths of the photo viewer (KINIG-87): zoom, pan and the fit size of a turned photo. Pure functions, no DOM: every number that
// comes from input (a wheel delta, a pointer position, a size from layout) may be NaN or huge, and none of it may reach the style.
//
// Coordinates: a point is in pixels from the centre of the stage (the area the photo is fitted into), x to the right, y down. The photo
// sits at the centre of the stage, is scaled by `zoom` around its own centre and moved by `pan`.

export interface Size {
  width: number;
  height: number;
}

export interface Point {
  x: number;
  y: number;
}

export interface View {
  zoom: number;
  pan: Point;
}

/** Fit size: the whole photo in the window. */
export const MIN_ZOOM = 1;
export const MAX_ZOOM = 8;
/** A double click zooms from fit to this. */
export const DOUBLE_ZOOM = 2;
/** The steps of the zoom buttons and keys, as multiples of fit. Wheel and pinch zoom between them. */
export const ZOOM_STEPS: readonly number[] = [1, 1.5, 2, 3, 4, 6, 8];

const STEP_EPSILON = 1e-6;
/** One pixel of wheel delta is this many powers of e of zoom; a mouse notch (100) is about 1.4×, a trackpad pinch tick (1 to 10) a percent or so. */
const WHEEL_SENSITIVITY = 0.0035;
const WHEEL_DELTA_LIMIT = 100;
const WHEEL_LINE_PX = 16;
const WHEEL_PAGE_PX = 100;

export const ORIGIN: Point = { x: 0, y: 0 };

const isUsable = (size: Size | null): size is Size =>
  size !== null && Number.isFinite(size.width) && Number.isFinite(size.height) && size.width > 0 && size.height > 0;

/** The zoom kept between fit and the maximum; anything that is not a number is fit. */
export function clampZoom(zoom: number): number {
  if (Number.isNaN(zoom)) return MIN_ZOOM;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
}

/** The next step of the ladder above (`1`) or below (`-1`) the current zoom; at the end of the ladder, the end. */
export function stepZoom(zoom: number, direction: -1 | 1): number {
  const current = clampZoom(zoom);
  if (direction === 1) {
    for (const step of ZOOM_STEPS) if (step > current + STEP_EPSILON) return step;
    return MAX_ZOOM;
  }
  for (const step of [...ZOOM_STEPS].reverse()) if (step < current - STEP_EPSILON) return step;
  return MIN_ZOOM;
}

/** The factor one `wheel` event zooms by. A delta that is not a number does nothing; a huge one counts as a big notch. */
export function wheelZoomFactor(deltaY: number, deltaMode: number): number {
  if (Number.isNaN(deltaY)) return 1;
  const unit = deltaMode === 1 ? WHEEL_LINE_PX : deltaMode === 2 ? WHEEL_PAGE_PX : 1;
  const delta = Math.max(-WHEEL_DELTA_LIMIT, Math.min(WHEEL_DELTA_LIMIT, deltaY * unit));
  return Math.exp(-delta * WHEEL_SENSITIVITY);
}

/**
 * The scale that fits the photo into the stage after `turns` quarter turns (a photo is never enlarged beyond its own size, like the old
 * `max-width: 100%`). `null` when a size is not known yet; the caller then lets CSS fit the photo.
 */
export function fitScale(natural: Size | null, stage: Size, turns: number): number | null {
  if (!isUsable(natural) || !isUsable(stage)) return null;
  const sideways = Math.abs(Math.trunc(turns)) % 2 === 1;
  const width = sideways ? natural.height : natural.width;
  const height = sideways ? natural.width : natural.height;
  return Math.min(1, stage.width / width, stage.height / height);
}

/** The size the photo takes on screen at fit (zoom 1) after the turns: the turned photo, scaled to fit. */
export function fittedSize(natural: Size | null, stage: Size, turns: number): Size | null {
  const scale = fitScale(natural, stage, turns);
  if (scale === null || !isUsable(natural)) return null;
  const sideways = Math.abs(Math.trunc(turns)) % 2 === 1;
  const width = sideways ? natural.height : natural.width;
  const height = sideways ? natural.width : natural.height;
  return { width: width * scale, height: height * scale };
}

/**
 * Keeps the photo from leaving the stage: while it is larger than the stage, no edge of it comes inside; while it is smaller, it stays
 * in the centre. `fitted` is the size at zoom 1; an unknown size counts as the stage itself.
 */
export function clampPan(pan: Point, zoom: number, fitted: Size | null, stage: Size): Point {
  const area = isUsable(stage) ? stage : { width: 0, height: 0 };
  const base = isUsable(fitted) ? fitted : area;
  const scale = clampZoom(zoom);
  const limit = (shown: number, room: number) => Math.max(0, (shown * scale - room) / 2);
  // `+ 0` turns a negative zero into a plain one.
  const along = (value: number, max: number) => (Number.isFinite(value) ? Math.min(max, Math.max(-max, value)) + 0 : 0);
  return {
    x: along(pan.x, limit(base.width, area.width)),
    y: along(pan.y, limit(base.height, area.height)),
  };
}

/**
 * Zooms so that the point of the photo under `anchor` stays under it, then keeps the photo on the stage. `anchor` is relative to
 * the stage centre; a missing or broken one is the centre.
 */
export function zoomAround(view: View, nextZoom: number, anchor: Point, fitted: Size | null, stage: Size): View {
  const zoom = clampZoom(nextZoom);
  const from = clampZoom(view.zoom);
  const ratio = zoom / from;
  const x = Number.isFinite(anchor.x) ? anchor.x : 0;
  const y = Number.isFinite(anchor.y) ? anchor.y : 0;
  const pan = { x: x - (x - view.pan.x) * ratio, y: y - (y - view.pan.y) * ratio };
  return { zoom, pan: clampPan(pan, zoom, fitted, stage) };
}

/** The anchor of a pointer at (`clientX`, `clientY`) for a stage with this bounding box. */
export function anchorOf(clientX: number, clientY: number, box: { left: number; top: number; width: number; height: number }): Point {
  const x = clientX - (box.left + box.width / 2);
  const y = clientY - (box.top + box.height / 2);
  return { x: Number.isFinite(x) ? x : 0, y: Number.isFinite(y) ? y : 0 };
}
