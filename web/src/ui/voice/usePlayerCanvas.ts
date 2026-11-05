import { useCallback, useEffect, useLayoutEffect, useRef, type RefObject } from "react";
import { drawPlayer, playerSize, snapLengths, type PlayerColors, type WaveGeometry } from "./waveDraw";
import { readWaveLengths } from "./waveTokens";

/** How many bars the player's waveform has (the number of peaks, `kinige_domain::attachment::WAVEFORM_PEAKS`). */
export const PLAYER_BARS = 64;

interface Measured {
  geometry: Omit<WaveGeometry, "color">;
  colors: PlayerColors;
}

/**
 * The player's waveform canvas (slice 7a, round 3). The canvas has one pixel per device pixel and its size is exactly the 64 bars and their
 * gaps in whole device pixels (`snapLengths` of the `layout.voice.wave-*` tokens), so the rhythm of the bars is even at every text size and
 * screen density; its CSS size is that many device pixels, set here (the stylesheet has the same size from the tokens as a first guess, so
 * nothing moves). `draw(progress)` repaints the row with the played part cut at `progress` (0 to 1, inside a bar too); it touches no React
 * state, so the animation frames of a playing note can call it as often as they like. Without peaks (not known yet, or the "could not
 * decode" marker) the row is flat dots. Where there is no 2D context (jsdom) nothing is drawn.
 */
export function usePlayerCanvas(peaks: Uint8Array | null): {
  canvas: RefObject<HTMLCanvasElement | null>;
  draw: (progress: number) => void;
  /** Reads the tokens and the density again (the text size and the screen can change): the player calls it about once a second while playing. */
  measure: () => void;
} {
  const canvas = useRef<HTMLCanvasElement>(null);
  const measured = useRef<Measured | null>(null);
  const peaksRef = useRef(peaks);
  const progress = useRef(0);

  const redraw = useCallback(() => {
    const element = canvas.current;
    const state = measured.current;
    const context = element?.getContext("2d") ?? null;
    if (!state || !context) return;
    drawPlayer(context, state.geometry, PLAYER_BARS, peaksRef.current, progress.current, state.colors);
  }, []);

  const measure = useCallback(() => {
    const element = canvas.current;
    if (!element) return;
    const lengths = readWaveLengths(element, "wave");
    if (!lengths) {
      measured.current = null;
      return;
    }
    const ratio = window.devicePixelRatio || 1;
    const snapped = snapLengths(lengths, ratio);
    const size = playerSize(PLAYER_BARS, snapped);
    if (element.width !== size.width) element.width = size.width;
    if (element.height !== size.height) element.height = size.height;
    // The canvas is as many CSS pixels wide as it has device pixels, divided by the density: one canvas pixel, one device pixel.
    element.style.width = `${size.width / ratio}px`;
    element.style.height = `${size.height / ratio}px`;
    const style = getComputedStyle(element);
    measured.current = {
      geometry: { ...snapped, width: size.width, height: size.height },
      colors: { played: style.color, unplayed: style.getPropertyValue("--color-voice-unplayed").trim() || style.color },
    };
    redraw();
  }, [redraw]);

  useLayoutEffect(() => {
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [measure]);

  // Until the tokens could be read (the stylesheet may not be applied at the first paint) every draw tries to measure again.
  useEffect(() => {
    peaksRef.current = peaks;
    if (measured.current === null) measure();
    else redraw();
  }, [peaks, redraw, measure]);

  const draw = useCallback(
    (share: number) => {
      progress.current = share;
      if (measured.current === null) measure();
      else redraw();
    },
    [redraw, measure],
  );

  return { canvas, draw, measure };
}
