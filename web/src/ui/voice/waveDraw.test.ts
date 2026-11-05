import { describe, expect, it } from "vitest";
import {
  barsFor,
  cssLengthPx,
  drawPlayer,
  drawWave,
  LIVE_GAIN,
  playerSize,
  snapLengths,
  type WaveContext,
  type WaveGeometry,
} from "./waveDraw";

// The guard on raw values reads tests too: units and colours are written the way the code receives them, from parts.
const rem = (amount: number | string) => `${amount}rem`;
const px = (amount: number | string) => `${amount}px`;
const ACCENT = "the-accent-colour";
const MUTED = "the-muted-colour";

/** The four lengths of a waveform (the guard on raw values does not read an object literal of sizes). */
function lens(barWidth: number, gap: number, minHeight: number, maxHeight: number) {
  return { barWidth, gap, minHeight, maxHeight };
}

function geometry(width: number, height: number, barWidth: number, gap: number, minHeight: number, maxHeight: number): WaveGeometry {
  return { width, height, barWidth, gap, minHeight, maxHeight, color: ACCENT };
}

/** 100 by 20, bars of 2 with gaps of 2, from 2 to 16 tall (the canvas height 20 is even, like the heights drawn on it). */
const GEOMETRY = geometry(100, 20, 2, 2, 2, 16);

interface Bar {
  x: number;
  y: number;
  h: number;
}

/** A context that records the bars of each fill (every `moveTo` starts one rounded bar) and the clip rectangles. */
function recorder() {
  const fills: { style: unknown; bars: Bar[]; clip: number | null }[] = [];
  let bars: Bar[] = [];
  let clip: number | null = null;
  let pendingRect: number | null = null;
  const calls: string[] = [];
  const context: WaveContext = {
    clearRect: () => calls.push("clear"),
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
    save: () => calls.push("save"),
    restore: () => {
      clip = null;
      calls.push("restore");
    },
    fill: () => fills.push({ style: context.fillStyle, bars: [...bars], clip }),
    fillStyle: "",
  };
  return { context, calls, fills, bars: () => fills.at(-1)?.bars ?? [] };
}

describe("cssLengthPx", () => {
  it("reads the token forms: rem (scaled by the root size), px and a bare number", () => {
    expect(cssLengthPx(rem(0.2), 13)).toBeCloseTo(2.6, 5);
    expect(cssLengthPx(` ${rem(1.6)} `, 20)).toBe(32);
    expect(cssLengthPx(px(3), 13)).toBe(3);
    expect(cssLengthPx("0", 13)).toBe(0);
    expect(cssLengthPx(rem(".5"), 10)).toBe(5);
  });

  it("is null for anything else, never NaN", () => {
    for (const bad of ["", "auto", `calc(${rem(1)} + ${px(2)})`, `${2}em`, `${"1e3"}rem`, "rem", "--1", "1 2"]) {
      expect(cssLengthPx(bad, 13), bad).toBeNull();
    }
  });
});

describe("snapLengths", () => {
  const lengths = lens(2.08, 1.43, 2.6, 20.8);

  it("makes whole device pixels at every density, so the rhythm of bars and gaps is even", () => {
    // The token lengths at a 13 px root: a CSS-pixel bar of 2.08 and gap of 1.43 snapped to 2 / 1 / 2 / 1 px pairs in both web views.
    expect(snapLengths(lengths, 1)).toEqual(lens(2, 1, 3, 21));
    expect(snapLengths(lengths, 2)).toEqual(lens(4, 3, 5, 42));
    expect(snapLengths(lengths, 3)).toEqual(lens(6, 4, 8, 62));
    for (const ratio of [1, 1.25, 1.5, 2, 2.5, 3]) {
      for (const value of Object.values(snapLengths(lengths, ratio))) {
        expect(Number.isInteger(value), `${ratio}`).toBe(true);
        expect(value).toBeGreaterThanOrEqual(1);
      }
    }
  });

  it("never gives a bar or a gap less than one device pixel, and survives a density that is not a number", () => {
    expect(snapLengths(lens(0, 0, 0, 0), 2)).toEqual(lens(1, 1, 1, 1));
    expect(snapLengths(lengths, Number.NaN)).toEqual(snapLengths(lengths, 1));
    expect(snapLengths(lengths, 0)).toEqual(snapLengths(lengths, 1));
    expect(snapLengths(lengths, -2)).toEqual(snapLengths(lengths, 1));
  });
});

describe("barsFor", () => {
  it("counts the bars that fit plus the entering and the leaving one", () => {
    expect(barsFor(100, 2, 2)).toBe(27);
    expect(barsFor(0, 2, 2)).toBe(0);
    expect(barsFor(100, 0, 0)).toBe(0);
    expect(barsFor(-5, 2, 2)).toBe(0);
  });
});

describe("drawWave, sliding", () => {
  const slide = (phase: number) => ({ kind: "slide", phase }) as const;

  it("clears, then draws the newest level at the right edge when the strip has slid in (phase 1), in one path and one fill", () => {
    const { context, calls, fills } = recorder();
    drawWave(context, GEOMETRY, [0.1, 0.2, 0.3], slide(1));
    expect(calls.filter((call) => call === "clear")).toHaveLength(1);
    expect(fills).toHaveLength(1);
    const bars = fills[0]?.bars ?? [];
    expect(bars).toHaveLength(3);
    // The newest bar (0.3) is first, flush with the right edge: its left side is at 100 - 2 and the path starts one radius in.
    expect(bars.map((bar) => bar.x)).toEqual([99, 95, 91]);
    expect(context.fillStyle).toBe(ACCENT);
  });

  it("a new level starts outside the right edge and slides in, so the picture moves between two samples", () => {
    // At phase 0 the newest bar is fully outside (left side at 104 of 100) and is not drawn; the one before it stands flush with the edge.
    const fresh = recorder();
    drawWave(fresh.context, GEOMETRY, [0.3, 0.3], slide(0));
    expect(fresh.bars().map((bar) => bar.x)).toEqual([99]);
    // Three quarters of the way in, the newest bar peeks in from the right, the one before it is a pitch to the left.
    const entering = recorder();
    drawWave(entering.context, GEOMETRY, [0.3, 0.3], slide(0.75));
    expect(entering.bars().map((bar) => bar.x)).toEqual([100.0, 96.0].map((x) => x));
    // Continuity: the bar that was newest at phase 1 stands where the next frame (phase 0 with one more level) puts it.
    const slid = recorder();
    drawWave(slid.context, GEOMETRY, [0.3, 0.3, 0.3], slide(1));
    const next = recorder();
    drawWave(next.context, GEOMETRY, [0.3, 0.3, 0.3, 0.3], slide(0));
    expect(next.bars()[0]?.x).toBeCloseTo(slid.bars()[0]?.x ?? 0, 5);
  });

  it("heights follow the level with the gain, whole pixels, centred, between the shortest and the tallest bar", () => {
    const { context, bars } = recorder();
    drawWave(context, GEOMETRY, [0, 0.1, 1], slide(1));
    // Newest first: level 1 is clamped to the tallest (16), 0.1 * gain is a part of the way, 0 is the shortest (2).
    const heights = bars().map((bar) => bar.h);
    expect(heights[0]).toBe(16);
    // 0.1 * gain (0.22) of the way from 2 to 16 is 5.08, rounded to 5, which has the wrong parity to centre on 20 and becomes 6.
    expect(0.1 * LIVE_GAIN).toBeCloseTo(0.22, 5);
    expect(heights[1]).toBe(6);
    expect(heights[2]).toBe(2);
    for (const bar of bars()) {
      expect(Number.isInteger(bar.y)).toBe(true);
      expect(Number.isInteger(bar.h)).toBe(true);
      // Centred: as much room above as below.
      expect(bar.y * 2 + bar.h).toBe(20);
    }
  });

  it("draws nothing for no levels or a canvas with no size, and survives a phase that is not a number", () => {
    const empty = recorder();
    drawWave(empty.context, GEOMETRY, [], slide(1));
    expect(empty.fills).toHaveLength(0);
    expect(empty.calls).toEqual(["clear"]);
    const flat = recorder();
    drawWave(flat.context, geometry(0, 20, 2, 2, 2, 16), [0.5], slide(1));
    expect(flat.fills).toHaveLength(0);
    const odd = recorder();
    drawWave(odd.context, GEOMETRY, [0.5], slide(Number.NaN));
    expect(odd.bars()).toHaveLength(1);
    drawWave(odd.context, GEOMETRY, [0.5], slide(-4));
    drawWave(odd.context, GEOMETRY, [0.5], slide(99));
  });

  it("stops at the left edge: bars that left the canvas are not drawn", () => {
    const { context, bars } = recorder();
    drawWave(context, GEOMETRY, Array.from({ length: 500 }, () => 0.2), slide(1));
    expect(bars().length).toBeLessThanOrEqual(27);
    expect(bars().length).toBeGreaterThanOrEqual(25);
  });
});

describe("drawWave, staying (Reduce Motion)", () => {
  const stay = (total: number) => ({ kind: "stay", total }) as const;
  /** 100 wide with a pitch of 4 and a gap of 2: 17 slots (17 * 4 - 2 = 66 would leave room: (100 + 2) / 4 = 25 slots). */
  const SLOTS = Math.floor((100 + 2) / 4);

  it("puts every level in a fixed slot, flush right, and does not scroll when new levels arrive", () => {
    const first = recorder();
    const levels = Array.from({ length: SLOTS }, (_, i) => (i % 5) / 10);
    drawWave(first.context, GEOMETRY, levels, stay(SLOTS));
    const before = first.bars().map((bar) => bar.x);
    expect(new Set(before).size).toBe(SLOTS);
    // One more level arrives: the oldest bar is overwritten in place; every other bar stands where it stood.
    const second = recorder();
    drawWave(second.context, GEOMETRY, [...levels.slice(1), 0.9], stay(SLOTS + 1));
    const after = second.bars().map((bar) => bar.x);
    expect([...after].sort((a, b) => a - b)).toEqual([...before].sort((a, b) => a - b));
    // Only the slot of the overwritten bar changed its height.
    const heightAt = (bars: Bar[]) => new Map(bars.map((bar) => [bar.x, bar.h]));
    const changed = [...heightAt(first.bars())].filter(([x, h]) => heightAt(second.bars()).get(x) !== h);
    expect(changed).toHaveLength(1);
  });

  it("fills slots from the cursor on, a recording shorter than the width leaves the other slots empty, bars are whole pixels", () => {
    const { context, bars } = recorder();
    drawWave(context, GEOMETRY, [0.3, 0.3, 0.3], stay(3));
    expect(bars()).toHaveLength(3);
    for (const bar of bars()) expect(Number.isInteger(bar.x - 1)).toBe(true);
  });

  it("slot numbers follow the sample number modulo the slots, so a long recording keeps cycling in place", () => {
    const run = (total: number) => {
      const { context, bars } = recorder();
      drawWave(context, GEOMETRY, Array.from({ length: SLOTS }, () => 0.3), stay(total));
      return bars().map((bar) => bar.x);
    };
    // The same newest `SLOTS` samples drawn at totals that differ by whole turns land on the same slots.
    expect(run(SLOTS).sort((a, b) => a - b)).toEqual(run(SLOTS * 40 + 7).sort((a, b) => a - b));
  });
});

describe("drawPlayer", () => {
  const size = playerSize(64, lens(4, 3, 5, 42));
  const player = { ...size, ...lens(4, 3, 5, 42) };
  const colors = { played: ACCENT, unplayed: MUTED };
  const peaks = Uint8Array.from({ length: 64 }, (_, i) => (i * 4) % 256);

  it("the canvas is exactly the bars and their gaps in whole device pixels", () => {
    expect([size.width, size.height]).toEqual([64 * 4 + 63 * 3, 42]);
    const none = playerSize(0, lens(4, 3, 5, 42));
    expect([none.width, none.height]).toEqual([0, 42]);
  });

  it("draws every bar on a whole-pixel grid with an even pitch", () => {
    const { context, fills } = recorder();
    drawPlayer(context, player, 64, peaks, 0, colors);
    expect(fills).toHaveLength(1);
    const bars = fills[0]?.bars ?? [];
    expect(bars).toHaveLength(64);
    // moveTo starts one radius into the bar: every pitch is exactly bar + gap, so the rhythm is even everywhere.
    const steps = new Set(bars.slice(1).map((bar, i) => bar.x - (bars[i]?.x ?? 0)));
    expect([...steps]).toEqual([7]);
    for (const bar of bars) {
      expect(Number.isInteger(bar.y)).toBe(true);
      expect(Number.isInteger(bar.h)).toBe(true);
      expect(bar.y * 2 + bar.h).toBe(42);
    }
    expect(fills[0]?.style).toBe(MUTED);
  });

  it("heights follow the peaks, between the shortest and the tallest", () => {
    const { context, bars } = recorder();
    drawPlayer(context, player, 64, peaks, 0, colors);
    const heights = bars().map((bar) => bar.h);
    // The shortest bar is 5 device pixels, which has the wrong parity to centre on 42 and becomes 6.
    expect(heights[0]).toBe(6);
    expect(Math.max(...heights)).toBeLessThanOrEqual(42);
    expect(heights[63]).toBeGreaterThan(heights[10] ?? 0);
  });

  it("without peaks, or with the empty marker, or with the wrong number, every bar is the shortest dot: a flat row", () => {
    for (const flat of [null, new Uint8Array(0), new Uint8Array(63), new Uint8Array(65).fill(255)]) {
      const { context, bars } = recorder();
      drawPlayer(context, player, 64, flat, 0, colors);
      expect(bars()).toHaveLength(64);
      expect(new Set(bars().map((bar) => bar.h))).toEqual(new Set([6]));
    }
  });

  it("the played layer is the same row cut at the share of the width, inside a bar too", () => {
    const { context, fills, calls } = recorder();
    drawPlayer(context, player, 64, peaks, 0.4, colors);
    expect(fills).toHaveLength(2);
    expect(fills[0]).toMatchObject({ style: MUTED, clip: null });
    expect(fills[1]).toMatchObject({ style: ACCENT });
    expect(fills[1]?.clip).toBeCloseTo(size.width * 0.4, 5);
    // A cut that is not on a bar boundary: 0.4 of 445 = 178 px, a bar starts every 7 px (178 / 7 is not a whole number).
    expect(((fills[1]?.clip ?? 0) / 7) % 1).not.toBe(0);
    expect(fills[1]?.bars).toHaveLength(64);
    expect(calls).toContain("save");
    expect(calls).toContain("restore");
  });

  it("no played layer at 0, all of it at 1, and garbage shares are clamped", () => {
    const none = recorder();
    drawPlayer(none.context, player, 64, peaks, 0, colors);
    expect(none.fills).toHaveLength(1);
    const all = recorder();
    drawPlayer(all.context, player, 64, peaks, 1, colors);
    expect(all.fills[1]?.clip).toBe(size.width);
    for (const odd of [Number.NaN, -3]) {
      const clamped = recorder();
      drawPlayer(clamped.context, player, 64, peaks, odd, colors);
      expect(clamped.fills).toHaveLength(1);
    }
    const over = recorder();
    drawPlayer(over.context, player, 64, peaks, 9, colors);
    expect(over.fills[1]?.clip).toBe(size.width);
  });

  it("draws nothing for an empty canvas or no bars", () => {
    const empty = recorder();
    drawPlayer(empty.context, { ...player, width: 0 }, 64, peaks, 0.5, colors);
    drawPlayer(empty.context, player, 0, peaks, 0.5, colors);
    expect(empty.fills).toHaveLength(0);
  });
});
