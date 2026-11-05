import { useEffect, useRef } from "react";
import { LEVEL_PERIOD_MS, type LiveLevels } from "../../voice/recorderSession";
import { barsFor, drawWave, snapLengths, type LiveMode, type WaveGeometry } from "./waveDraw";
import { readWaveLengths } from "./waveTokens";

interface LiveWaveProps {
  /** The newest `count` levels of the recording (0 to 1, oldest first), when the newest was taken and how many there have been. Read on every frame; never a render. */
  read: (count: number) => LiveLevels;
  /** The accessible name of the waveform ("Microphone level"). */
  label: string;
}

/**
 * The live waveform of the recording bar (slice 7a): a canvas that fills the whole middle of the bar, between the timer and Send. New bars
 * enter from the right and scroll to the left. The canvas redraws on its own animation frames, reading the recorder through `read`, so a
 * recording never re-renders the bar, let alone the feed (KINIG-47). The canvas has one pixel per device pixel and the bars are whole device
 * pixels (an even rhythm at every density). With Reduce Motion nothing scrolls: the bars stand in fixed slots across the width and only
 * their heights change, ten times a second (the oldest bar is the one that is overwritten). Lengths and the colour come from the design
 * tokens through the computed style.
 */
export function LiveWave({ read, label }: LiveWaveProps) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const element = canvas.current;
    const context = element?.getContext("2d") ?? null;
    if (!element || !context) return;

    let geometry: WaveGeometry | null = null;
    let tokensAt = performance.now();

    /** Measures the element and the tokens; the canvas gets its pixel size (the box in device pixels) only when the box changed. */
    const measure = (resizeCanvas: boolean) => {
      const box = element.getBoundingClientRect();
      const ratio = window.devicePixelRatio || 1;
      if (resizeCanvas) {
        element.width = Math.max(1, Math.round(box.width * ratio));
        element.height = Math.max(1, Math.round(box.height * ratio));
      }
      const lengths = readWaveLengths(element, "rec-wave");
      geometry = lengths
        ? { ...snapLengths(lengths, ratio), width: element.width, height: element.height, color: getComputedStyle(element).color }
        : null;
    };
    measure(true);

    const reduced = prefersReducedMotion(element);
    const frame = () => {
      // The text size and the colours can change (the settings window): look at the tokens again about once a second. Until the tokens could
      // be read at all (the stylesheet may not be applied at the first frame) look again on every frame, so the bars are never missing for a second.
      const now = performance.now();
      if (!geometry || now - tokensAt > 1000) {
        tokensAt = now;
        measure(geometry === null);
      }
      if (!geometry) return;
      const { levels, at, total } = read(barsFor(geometry.width, geometry.barWidth, geometry.gap));
      const mode: LiveMode = reduced ? { kind: "stay", total } : { kind: "slide", phase: (now - at) / LEVEL_PERIOD_MS };
      drawWave(context, geometry, levels, mode);
    };

    // Resizing a canvas clears it: draw again at once (the layout settles after the first paint, e.g. when the timer's font loads), so the bars
    // are never missing until the next frame or step.
    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(() => {
            measure(true);
            frame();
          });
    observer?.observe(element);

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

function prefersReducedMotion(element: HTMLElement): boolean {
  const media = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  return media || element.closest("[data-reduce-motion]") !== null;
}
