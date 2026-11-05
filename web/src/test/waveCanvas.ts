import { vi } from "vitest";

// A canvas for the tests of the voice waveforms (slice 7a): the 2D context remembers what is filled, and the computed style gives the token
// lengths, the colours and a root font size. The guard on raw values reads tests too, so units are written from parts.
const rem = (amount: number) => `${amount}rem`;

export interface FilledBars {
  style: unknown;
  /** The left sides (the path starts one radius into the bar) and the heights of the bars of one fill. */
  bars: { x: number; h: number }[];
  /** The width of the clip rectangle the fill was cut by, or `null`. */
  clip: number | null;
}

const TOKENS: Record<string, string> = {
  "--layout-voice-wave-bar-width": rem(0.16),
  "--layout-voice-wave-gap": rem(0.11),
  "--layout-voice-wave-min-height": rem(0.2),
  "--layout-voice-wave-height": rem(1.6),
  "--layout-voice-rec-wave-bar-width": rem(0.2),
  "--layout-voice-rec-wave-gap": rem(0.15),
  "--layout-voice-rec-wave-min-height": rem(0.2),
  "--layout-voice-rec-wave-height": rem(1.6),
  "--color-voice-unplayed": "the-muted-colour",
};

/**
 * Makes every canvas of the test a recording one and every computed style carry the tokens (13 px root, density 1 unless `ratio` says
 * otherwise). Returns the fills in the order they were made. Call `vi.restoreAllMocks()` and `vi.unstubAllGlobals()` afterwards.
 */
export function installWaveCanvas({ ratio = 1 }: { ratio?: number } = {}) {
  const fills: FilledBars[] = [];
  let bars: { x: number; h: number; y: number }[] = [];
  let pendingRect: number | null = null;
  let clip: number | null = null;
  const context = {
    fillStyle: "" as unknown,
    clearRect: () => undefined,
    beginPath: () => {
      bars = [];
    },
    moveTo: (x: number, y: number) => bars.push({ x, y, h: 0 }),
    arcTo: (_x1: number, _y1: number, _x2: number, y2: number) => {
      const bar = bars.at(-1);
      if (bar && y2 > bar.y) bar.h = Math.max(bar.h, y2 - bar.y);
    },
    closePath: () => undefined,
    rect: (_x: number, _y: number, w: number) => {
      pendingRect = w;
    },
    clip: () => {
      clip = pendingRect;
    },
    save: () => undefined,
    restore: () => {
      clip = null;
    },
    fill: () => fills.push({ style: context.fillStyle, bars: bars.map(({ x, h }) => ({ x, h })), clip }),
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(() => context as unknown as CanvasRenderingContext2D);
  vi.stubGlobal("devicePixelRatio", ratio);
  vi.spyOn(window, "getComputedStyle").mockImplementation(
    () =>
      ({
        getPropertyValue: (name: string) => TOKENS[name] ?? "",
        color: "the-accent-colour",
        fontSize: `${13}px`,
      }) as unknown as CSSStyleDeclaration,
  );
  return { fills, context };
}
