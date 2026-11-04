// The drawing of the recording bar's live waveform (slice 7a, round 2): thin rounded bars that enter from the right and scroll to the left,
// one new bar every `LEVEL_PERIOD_MS`. Pure functions on a canvas context, so the geometry can be tested without a canvas.

/** The analyser's levels sit low (speech is about 0.2 to 0.4 of the full scale after the square root), so the bars are drawn this much taller. */
export const LIVE_GAIN = 2.2;

/** What the canvas needs to know to draw, in CSS pixels. The lengths come from the design tokens (`cssLengthPx`), the colour from the computed style. */
export interface WaveGeometry {
  width: number;
  height: number;
  barWidth: number;
  gap: number;
  minHeight: number;
  maxHeight: number;
  color: string;
}

/** The part of a 2D context the drawing uses (a test passes a recorder). */
export type WaveContext = Pick<
  CanvasRenderingContext2D,
  "clearRect" | "beginPath" | "moveTo" | "arcTo" | "closePath" | "fill" | "fillStyle"
>;

/** A token length ("0.2rem", "3px", "0") in pixels, or `null` for anything else. `rootPx` is the root font size (the text size setting scales it). */
export function cssLengthPx(value: string, rootPx: number): number | null {
  const text = value.trim();
  const match = /^(-?\d*\.?\d+)(rem|px)?$/.exec(text);
  if (!match) return null;
  const amount = Number(match[1]);
  if (!Number.isFinite(amount)) return null;
  return match[2] === "rem" ? amount * rootPx : amount;
}

/** How many bars fit into `width`, plus the one that is entering and the one that is leaving. */
export function barsFor(width: number, barWidth: number, gap: number): number {
  const pitch = barWidth + gap;
  return pitch > 0 && width > 0 ? Math.ceil(width / pitch) + 2 : 0;
}

/**
 * Draws the newest levels (0 to 1, oldest first) as one rounded bar each. `phase` (0 to 1) is how far the strip has scrolled since the
 * newest level was taken: at 0 the newest bar is just outside the right edge, at 1 it stands flush with it, so the picture moves smoothly
 * between two samples and a new sample continues exactly where the last frame ended. Everything is one path and one fill.
 */
export function drawWave(context: WaveContext, geometry: WaveGeometry, levels: readonly number[], phase: number): void {
  const { width, height, barWidth, gap, minHeight, maxHeight } = geometry;
  context.clearRect(0, 0, width, height);
  if (levels.length === 0 || width <= 0 || height <= 0) return;
  const pitch = barWidth + gap;
  const share = Math.min(1, Math.max(0, Number.isFinite(phase) ? phase : 1));
  const radius = barWidth / 2;
  context.beginPath();
  for (let k = 0; k < levels.length; k += 1) {
    const right = width + (1 - share) * pitch - k * pitch;
    const left = right - barWidth;
    if (right < 0) break;
    if (left > width) continue;
    // The newest level is the last of the array, so bar `k` counts from the end.
    const level = levels[levels.length - 1 - k] ?? 0;
    const scaled = Math.min(1, Math.max(0, level * LIVE_GAIN));
    const barHeight = Math.min(maxHeight, minHeight + (maxHeight - minHeight) * scaled);
    const top = (height - barHeight) / 2;
    roundedBar(context, left, top, barWidth, barHeight, Math.min(radius, barHeight / 2));
  }
  context.fillStyle = geometry.color;
  context.fill();
}

function roundedBar(context: WaveContext, x: number, y: number, w: number, h: number, r: number): void {
  context.moveTo(x + r, y);
  context.arcTo(x + w, y, x + w, y + h, r);
  context.arcTo(x + w, y + h, x, y + h, r);
  context.arcTo(x, y + h, x, y, r);
  context.arcTo(x, y, x + w, y, r);
  context.closePath();
}
