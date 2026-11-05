import { cssLengthPx, type WaveLengths } from "./waveDraw";

// Reading the design tokens from the computed style for the canvases (slice 7a): the lengths of the bars are `layout.voice.*` tokens in rem,
// so they follow the text size; the canvas turns them into whole device pixels (`snapLengths`).

/** The root font size in CSS pixels (the text size setting scales it). */
export function rootFontPx(): number {
  return Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
}

/** The four lengths of a waveform from tokens named `--layout-voice-<prefix>-bar-width`, `-gap`, `-min-height` and `-height`; `null` where one is not a length. */
export function readWaveLengths(element: HTMLElement, prefix: string): WaveLengths | null {
  const style = getComputedStyle(element);
  const rootPx = rootFontPx();
  const length = (suffix: string) => cssLengthPx(style.getPropertyValue(`--layout-voice-${prefix}-${suffix}`), rootPx);
  const barWidth = length("bar-width");
  const gap = length("gap");
  const minHeight = length("min-height");
  const maxHeight = length("height");
  if (barWidth === null || gap === null || minHeight === null || maxHeight === null) return null;
  return { barWidth, gap, minHeight, maxHeight };
}
