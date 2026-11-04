import { describe, expect, it } from "vitest";
import { barsFor, cssLengthPx, drawWave, LIVE_GAIN, type WaveContext, type WaveGeometry } from "./waveDraw";

// The guard on raw values reads tests too: units and colours are written the way the code receives them, from parts.
const rem = (amount: number | string) => `${amount}rem`;
const px = (amount: number | string) => `${amount}px`;
const ACCENT = "the-accent-colour";

function geometry(width: number, height: number, barWidth: number, gap: number, minHeight: number, maxHeight: number): WaveGeometry {
  return { width, height, barWidth, gap, minHeight, maxHeight, color: ACCENT };
}

/** 100 by 20, bars of 2 with gaps of 2, from 2 to 16 tall. */
const GEOMETRY = geometry(100, 20, 2, 2, 2, 16);

/** A context that records the bars: every `moveTo` after `beginPath` starts one rounded bar. */
function recorder() {
  const calls: string[] = [];
  const bars: { x: number; y: number; h: number }[] = [];
  const context: WaveContext = {
    clearRect: () => calls.push("clear"),
    beginPath: () => calls.push("begin"),
    moveTo: (x: number, y: number) => {
      calls.push("moveTo");
      bars.push({ x, y, h: 0 });
    },
    arcTo: (_x1: number, _y1: number, _x2: number, y2: number) => {
      const bar = bars.at(-1);
      if (bar && y2 > bar.y) bar.h = Math.max(bar.h, y2 - bar.y);
    },
    closePath: () => calls.push("close"),
    fill: () => calls.push("fill"),
    fillStyle: "",
  };
  return { context, calls, bars };
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

describe("barsFor", () => {
  it("counts the bars that fit plus the entering and the leaving one", () => {
    expect(barsFor(100, 2, 2)).toBe(27);
    expect(barsFor(0, 2, 2)).toBe(0);
    expect(barsFor(100, 0, 0)).toBe(0);
    expect(barsFor(-5, 2, 2)).toBe(0);
  });
});

describe("drawWave", () => {
  it("clears, then draws the newest level at the right edge when the strip has slid in (phase 1), in one path and one fill", () => {
    const { context, calls, bars } = recorder();
    drawWave(context, GEOMETRY, [0.1, 0.2, 0.3], 1);
    expect(calls.filter((call) => call === "clear")).toHaveLength(1);
    expect(calls.filter((call) => call === "begin")).toHaveLength(1);
    expect(calls.filter((call) => call === "fill")).toHaveLength(1);
    expect(bars).toHaveLength(3);
    // The newest bar (0.3) is first, flush with the right edge: its left side is at 100 - 2 and the path starts one radius in.
    expect(bars[0]?.x).toBeCloseTo(98 + 1, 5);
    expect(bars[1]?.x).toBeCloseTo(94 + 1, 5);
    expect(bars[2]?.x).toBeCloseTo(90 + 1, 5);
    expect(context.fillStyle).toBe(ACCENT);
  });

  it("a new level starts outside the right edge and slides in, so the picture moves between two samples", () => {
    // At phase 0 the newest bar is fully outside (left side at 102 of 100) and is not drawn; the one before it stands flush with the edge.
    const fresh = recorder();
    drawWave(fresh.context, GEOMETRY, [0.3, 0.3], 0);
    expect(fresh.bars).toHaveLength(1);
    expect(fresh.bars[0]?.x).toBeCloseTo(98 + 1, 5);
    // A quarter of the way in, the newest bar peeks in from the right (left side at 99), the one before it is a pitch to the left.
    const entering = recorder();
    drawWave(entering.context, GEOMETRY, [0.3, 0.3], 0.75);
    expect(entering.bars[0]?.x).toBeCloseTo(99 + 1, 5);
    expect(entering.bars[1]?.x).toBeCloseTo(95 + 1, 5);
    // Continuity: the bar that was newest at phase 1 stands where the next frame (phase 0 with one more level) puts it.
    const slid = recorder();
    drawWave(slid.context, GEOMETRY, [0.3, 0.3, 0.3], 1);
    const next = recorder();
    drawWave(next.context, GEOMETRY, [0.3, 0.3, 0.3, 0.3], 0);
    expect(next.bars[0]?.x).toBeCloseTo(slid.bars[0]?.x ?? 0, 5);
  });

  it("heights follow the level with the gain, between the shortest and the tallest bar", () => {
    const { context, bars } = recorder();
    drawWave(context, GEOMETRY, [0, 0.1, 1], 1);
    // Newest first: level 1 is clamped to the tallest (16), 0.1 * gain is a part of the way, 0 is the shortest (2).
    expect(bars[0]?.h).toBeCloseTo(16, 5);
    expect(bars[1]?.h).toBeCloseTo(2 + 14 * Math.min(1, 0.1 * LIVE_GAIN), 5);
    expect(bars[2]?.h).toBeCloseTo(2, 5);
    // Bars are centred vertically.
    expect(bars[0]?.y).toBeCloseTo(2, 5);
  });

  it("draws nothing for no levels or a canvas with no size, and survives a phase that is not a number", () => {
    const empty = recorder();
    drawWave(empty.context, GEOMETRY, [], 1);
    expect(empty.bars).toHaveLength(0);
    expect(empty.calls).toEqual(["clear"]);
    const flat = recorder();
    drawWave(flat.context, geometry(0, 20, 2, 2, 2, 16), [0.5], 1);
    expect(flat.bars).toHaveLength(0);
    const odd = recorder();
    drawWave(odd.context, GEOMETRY, [0.5], Number.NaN);
    expect(odd.bars).toHaveLength(1);
    drawWave(odd.context, GEOMETRY, [0.5], -4);
    drawWave(odd.context, GEOMETRY, [0.5], 99);
  });

  it("stops at the left edge: bars that left the canvas are not drawn", () => {
    const { context, bars } = recorder();
    drawWave(context, GEOMETRY, Array.from({ length: 500 }, () => 0.2), 1);
    expect(bars.length).toBeLessThanOrEqual(27);
    expect(bars.length).toBeGreaterThanOrEqual(25);
  });
});
