import { describe, expect, it } from "vitest";
import { fontPx, pickMonths, weekdayStep } from "./labels";

// Which edge labels the board draws: decided in screen pixels against the font size, so they scale with large text.
const FULL = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const spots = (pitch: number) => FULL.map((name, i) => ({ x: i * pitch, full: name, narrow: name.slice(0, 1) }));
/** 7 px per character, like a small label font. */
const measure = (text: string) => text.length * 7;

describe("fontPx", () => {
  it("reads the size from a canvas font string and has a fallback", () => {
    expect(fontPx(`400 ${13}px Inter, sans-serif`)).toBe(13);
    expect(fontPx(`500 ${16.25}px Inter`)).toBe(16.25);
    expect(fontPx("")).toBe(13);
    expect(fontPx(`bold ${0}px x`)).toBe(13);
  });
});

describe("weekdayStep", () => {
  it("every row when they are apart, every other when close, none when they would pile up", () => {
    expect(weekdayStep(40, 13)).toBe(1);
    expect(weekdayStep(12, 13)).toBe(2);
    expect(weekdayStep(7, 13)).toBe(0);
  });

  it("asks for more room at large text", () => {
    expect(weekdayStep(18, 13)).toBe(1);
    expect(weekdayStep(18, 16.25)).toBe(2);
  });
});

describe("pickMonths", () => {
  it("draws the full names when they all fit", () => {
    expect(pickMonths(spots(60), measure).map((m) => m.text)).toEqual(FULL);
  });

  it("falls back to one letter each when the names touch", () => {
    const picked = pickMonths(spots(20), measure);
    expect(picked.map((m) => m.text)).toEqual(FULL.map((name) => name.slice(0, 1)));
  });

  it("leaves out the letters that still touch, but never everything: a narrow window keeps some months", () => {
    const picked = pickMonths(spots(9), measure);
    expect(picked.length).toBeGreaterThan(2);
    expect(picked.length).toBeLessThan(12);
    for (let i = 1; i < picked.length; i++) expect(picked[i]!.spot.x - picked[i - 1]!.spot.x).toBeGreaterThanOrEqual(11);
  });

  it("garbage widths and empty names do not throw", () => {
    expect(() => pickMonths(spots(10), () => NaN)).not.toThrow();
    expect(pickMonths([], measure)).toEqual([]);
    expect(pickMonths([{ x: 0, full: "", narrow: "" }], measure)).toEqual([]);
  });
});
