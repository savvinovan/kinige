import { useEffect, useRef } from "react";
import { LEVEL_PERIOD_MS, type LiveLevels } from "../../voice/recorderSession";
import { barsFor, cssLengthPx, drawWave, type WaveGeometry } from "./waveDraw";

interface LiveWaveProps {
  /** The newest `count` levels of the recording (0 to 1, oldest first) and when the newest was taken. Read on every frame; never a render. */
  read: (count: number) => LiveLevels;
  /** The accessible name of the waveform ("Microphone level"). */
  label: string;
}

/**
 * The live waveform of the recording bar (slice 7a, round 2): a canvas that fills the whole middle of the bar, between the timer and Send.
 * New bars enter from the right and scroll to the left. The canvas redraws on its own animation frames, reading the recorder through `read`,
 * so a recording never re-renders the bar, let alone the feed (KINIG-47). With Reduce Motion the strip does not slide: it steps once per
 * 100 ms, a level meter that shows the loudness and nothing else. Colours and lengths come from the design tokens through the computed style.
 */
export function LiveWave({ read, label }: LiveWaveProps) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const element = canvas.current;
    const context = element?.getContext("2d") ?? null;
    if (!element || !context) return;

    let geometry: WaveGeometry | null = null;
    let tokensAt = performance.now();

    /** Measures the element and the tokens; the canvas gets its pixel size only when the box changed. */
    const measure = (resizeCanvas: boolean) => {
      const box = element.getBoundingClientRect();
      if (resizeCanvas) {
        const scale = window.devicePixelRatio || 1;
        element.width = Math.max(1, Math.round(box.width * scale));
        element.height = Math.max(1, Math.round(box.height * scale));
        context.setTransform(scale, 0, 0, scale, 0, 0);
      }
      const tokens = readTokens(element);
      geometry = tokens ? { ...tokens, width: box.width, height: box.height } : null;
    };
    measure(true);
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(() => measure(true));
    observer?.observe(element);

    const reduced = prefersReducedMotion(element);
    const frame = () => {
      // The text size and the colours can change (the settings window): look at the tokens again about once a second.
      const now = performance.now();
      if (now - tokensAt > 1000) {
        tokensAt = now;
        measure(false);
      }
      if (!geometry) return;
      const { levels, at } = read(barsFor(geometry.width, geometry.barWidth, geometry.gap));
      const phase = reduced ? 1 : (now - at) / LEVEL_PERIOD_MS;
      drawWave(context, geometry, levels, phase);
    };

    let handle = 0;
    let timer: ReturnType<typeof setInterval> | null = null;
    const loop = () => {
      frame();
      handle = requestAnimationFrame(loop);
    };
    if (reduced) {
      frame();
      timer = setInterval(frame, LEVEL_PERIOD_MS * 2);
    } else {
      handle = requestAnimationFrame(loop);
    }
    return () => {
      cancelAnimationFrame(handle);
      if (timer !== null) clearInterval(timer);
      observer?.disconnect();
    };
  }, [read]);

  return <canvas ref={canvas} className="recording-bar__wave" role="img" aria-label={label} />;
}

/** The token lengths and the colour as the element computes them; `null` where a length is not one we understand (nothing is drawn then). */
function readTokens(element: HTMLElement): Omit<WaveGeometry, "width" | "height"> | null {
  const style = getComputedStyle(element);
  const rootPx = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
  const length = (name: string) => cssLengthPx(style.getPropertyValue(name), rootPx);
  const barWidth = length("--layout-voice-rec-wave-bar-width");
  const gap = length("--layout-voice-rec-wave-gap");
  const minHeight = length("--layout-voice-rec-wave-min-height");
  const maxHeight = length("--layout-voice-rec-wave-height");
  if (barWidth === null || gap === null || minHeight === null || maxHeight === null) return null;
  return { barWidth, gap, minHeight, maxHeight, color: style.color };
}

function prefersReducedMotion(element: HTMLElement): boolean {
  const media = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  return media || element.closest("[data-reduce-motion]") !== null;
}
