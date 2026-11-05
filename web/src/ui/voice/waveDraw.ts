// The drawing of the voice waveforms (slice 7a, round 3): the recording bar's live waveform and the player's waveform. Pure functions on a
// canvas context, so the geometry can be tested without a canvas. Both are drawn in DEVICE pixels (the canvas has one pixel per device
// pixel): a bar and a gap are whole numbers of device pixels, so the rhythm is even at every text size and screen density. (As CSS lengths
// the bars were 2.08 px with 1.43 px gaps and both web views snapped them to 2 / 1 / 2 / 1 px pairs.)

/** The analyser's levels sit low (speech is about 0.2 to 0.4 of the full scale after the square root), so the bars are drawn this much taller. */
export const LIVE_GAIN = 2.2;

/** The part of a 2D context the drawing uses (a test passes a recorder). */
export type WaveContext = Pick<
  CanvasRenderingContext2D,
  "clearRect" | "beginPath" | "moveTo" | "arcTo" | "closePath" | "fill" | "fillStyle" | "save" | "restore" | "rect" | "clip"
>;

/** A token length ("0.2rem", "3px", "0") in CSS pixels, or `null` for anything else. `rootPx` is the root font size (the text size setting scales it). */
export function cssLengthPx(value: string, rootPx: number): number | null {
  const text = value.trim();
  const match = /^(-?\d*\.?\d+)(rem|px)?$/.exec(text);
  if (!match) return null;
  const amount = Number(match[1]);
  if (!Number.isFinite(amount)) return null;
  return match[2] === "rem" ? amount * rootPx : amount;
}

/** Token lengths in CSS pixels, as read from the computed style. */
export interface WaveLengths {
  barWidth: number;
  gap: number;
  minHeight: number;
  maxHeight: number;
}

/** What a canvas draws with, in DEVICE pixels, all whole numbers. */
export interface WaveGeometry {
  width: number;
  height: number;
  barWidth: number;
  gap: number;
  minHeight: number;
  maxHeight: number;
  color: string;
}

/** Whole device pixels, never fewer than one. */
const device = (css: number, ratio: number): number => Math.max(1, Math.round(css * ratio));

/** The lengths as whole device pixels at `ratio` device pixels per CSS pixel (non-numbers count as 1). */
export function snapLengths(lengths: WaveLengths, ratio: number): Omit<WaveGeometry, "width" | "height" | "color"> {
  const scale = Number.isFinite(ratio) && ratio > 0 ? ratio : 1;
  return {
    barWidth: device(lengths.barWidth, scale),
    gap: device(lengths.gap, scale),
    minHeight: device(lengths.minHeight, scale),
    maxHeight: device(lengths.maxHeight, scale),
  };
}

/** How many bars fit into `width`, plus the one that is entering and the one that is leaving. */
export function barsFor(width: number, barWidth: number, gap: number): number {
  const pitch = barWidth + gap;
  return pitch > 0 && width > 0 ? Math.ceil(width / pitch) + 2 : 0;
}

/** The mode of the live waveform: the strip slides (`phase`, 0 to 1), or, with Reduce Motion, the bars stay in their slots (`ringTotal`). */
export type LiveMode = { kind: "slide"; phase: number } | { kind: "stay"; total: number };

/**
 * Draws the newest levels (0 to 1, oldest first) as one rounded bar each, in one path and one fill.
 * - `slide`: the newest bar enters from the right and the strip scrolls to the left. `phase` (0 to 1) is how far it has slid since the newest
 *   level was taken: at 0 the newest bar is just outside the right edge, at 1 it stands flush with it, so the picture moves smoothly between
 *   two samples and a new sample continues exactly where the last frame ended.
 * - `stay` (Reduce Motion): as many bars as fit stand in fixed slots across the width and nothing scrolls; sample number `j` goes to slot
 *   `j` modulo the number of slots, so a new level only changes the height of the bar it overwrites (the oldest).
 */
export function drawWave(context: WaveContext, geometry: WaveGeometry, levels: readonly number[], mode: LiveMode): void {
  const { width, height, barWidth, gap } = geometry;
  context.clearRect(0, 0, width, height);
  if (levels.length === 0 || width <= 0 || height <= 0) return;
  const pitch = barWidth + gap;
  context.beginPath();
  if (mode.kind === "slide") {
    const share = Math.min(1, Math.max(0, Number.isFinite(mode.phase) ? mode.phase : 1));
    for (let k = 0; k < levels.length; k += 1) {
      const right = width + (1 - share) * pitch - k * pitch;
      const left = right - barWidth;
      if (right < 0) break;
      if (left > width) continue;
      // The newest level is the last of the array, so bar `k` counts from the end.
      liveBar(context, geometry, left, levels[levels.length - 1 - k] ?? 0);
    }
  } else {
    const slots = Math.max(1, Math.floor((width + gap) / pitch));
    // Slots are flush with the right edge, like the last bar of the sliding strip.
    const first = width - (slots * pitch - gap);
    const oldest = mode.total - levels.length;
    for (let k = Math.max(0, levels.length - slots); k < levels.length; k += 1) {
      const slot = (oldest + k) % slots;
      liveBar(context, geometry, first + slot * pitch, levels[k] ?? 0);
    }
  }
  context.fillStyle = geometry.color;
  context.fill();
}

function liveBar(context: WaveContext, g: WaveGeometry, left: number, level: number): void {
  const scaled = Math.min(1, Math.max(0, Number.isFinite(level) ? level * LIVE_GAIN : 0));
  roundedBar(context, g, left, g.minHeight + (g.maxHeight - g.minHeight) * scaled);
}

/** The colours of the player's two layers. */
export interface PlayerColors {
  played: string;
  unplayed: string;
}

/** The player's waveform: how many bars, and the device-pixel size of the canvas they exactly fill. */
export function playerSize(bars: number, g: Pick<WaveGeometry, "barWidth" | "gap" | "maxHeight">): { width: number; height: number } {
  return { width: Math.max(0, bars * g.barWidth + Math.max(0, bars - 1) * g.gap), height: g.maxHeight };
}

/**
 * Draws the player's waveform: `bars` bars of `peaks` (0 to 255; with none, or the wrong number, or the empty "could not decode" marker,
 * every bar is a dot of the shortest height, a flat row, as Telegram draws while a note loads), the whole row in the unplayed colour and
 * the part left of `progress` (0 to 1 of the width) in the played colour. The played layer is the same row cut at `progress * width` with a
 * clip, so the fill stops inside a bar, not only between bars.
 */
export function drawPlayer(
  context: WaveContext,
  geometry: Omit<WaveGeometry, "color">,
  bars: number,
  peaks: Uint8Array | null,
  progress: number,
  colors: PlayerColors,
): void {
  const { width, height } = geometry;
  context.clearRect(0, 0, width, height);
  if (width <= 0 || height <= 0 || bars <= 0) return;
  const shape = peaks !== null && peaks.length === bars ? peaks : null;
  const trace = () => {
    context.beginPath();
    for (let i = 0; i < bars; i += 1) {
      const peak = shape ? (shape[i] ?? 0) / 255 : 0;
      roundedBar(context, geometry, i * (geometry.barWidth + geometry.gap), geometry.minHeight + (geometry.maxHeight - geometry.minHeight) * peak);
    }
  };
  trace();
  context.fillStyle = colors.unplayed;
  context.fill();
  const share = Math.min(1, Math.max(0, Number.isFinite(progress) ? progress : 0));
  if (share <= 0) return;
  context.save();
  context.beginPath();
  context.rect(0, 0, width * share, height);
  context.clip();
  trace();
  context.fillStyle = colors.played;
  context.fill();
  context.restore();
}

/**
 * One rounded bar at `left`, centred vertically. The height is a whole number with the parity of the canvas height, so the top is a whole
 * pixel too; the radius is half the width (the full roundness a bar of that width can have).
 */
function roundedBar(context: WaveContext, g: Omit<WaveGeometry, "color">, left: number, wanted: number): void {
  let h = Math.min(g.maxHeight, Math.max(g.minHeight, Math.round(wanted)));
  if ((g.height - h) % 2 !== 0) h = h + 1 <= g.maxHeight ? h + 1 : h - 1;
  const top = (g.height - h) / 2;
  const w = g.barWidth;
  const r = Math.min(w / 2, h / 2);
  context.moveTo(left + r, top);
  context.arcTo(left + w, top, left + w, top + h, r);
  context.arcTo(left + w, top + h, left, top + h, r);
  context.arcTo(left, top + h, left, top, r);
  context.arcTo(left, top, left + w, top, r);
  context.closePath();
}
