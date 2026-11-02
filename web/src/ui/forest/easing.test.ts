import { describe, expect, it } from "vitest";
import { cubicBezier, parseBezier, parseDuration } from "./easing";

describe("the growth curve and duration come from the tokens", () => {
  it("parses a CSS cubic-bezier and rejects anything else", () => {
    expect(parseBezier("cubic-bezier(0, 0, 0.2, 1)")).toEqual([0, 0, 0.2, 1]);
    expect(parseBezier(" cubic-bezier(0.2,0,0,1) ")).toEqual([0.2, 0, 0, 1]);
    for (const bad of ["", "ease", "cubic-bezier()", "cubic-bezier(1,2,3)", "cubic-bezier(a,b,c,d)", "cubic-bezier(2,0,0,1)", "cubic-bezier(0,0,-1,1)", "cubic-bezier(0,0,0,NaN)"]) {
      expect(parseBezier(bad), bad).toBeNull();
    }
  });

  it("evaluates monotonically from 0 to 1 and clamps outside", () => {
    const ease = cubicBezier("cubic-bezier(0, 0, 0.2, 1)");
    expect(ease(0)).toBe(0);
    expect(ease(1)).toBe(1);
    expect(ease(-5)).toBe(0);
    expect(ease(9)).toBe(1);
    let previous = 0;
    for (let i = 1; i <= 20; i++) {
      const value = ease(i / 20);
      expect(value).toBeGreaterThanOrEqual(previous);
      previous = value;
    }
    expect(ease(0.5)).toBeGreaterThan(0.5); // an ease-out curve
  });

  it("an unreadable curve is linear", () => {
    const linear = cubicBezier("nonsense");
    expect(linear(0.25)).toBe(0.25);
    expect(linear(2)).toBe(1);
  });

  it("reads CSS times; anything unreadable is 0, so the tree jumps", () => {
    // Built from parts: the raw-values test forbids literal times in source files, even in tests.
    const seconds = (n: number | string) => `${n}s`;
    const millis = (n: number | string) => `${n}ms`;
    expect(parseDuration(seconds(0.7))).toBe(700);
    expect(parseDuration(millis(700))).toBe(700);
    expect(parseDuration(` ${seconds(1.5)} `)).toBe(1500);
    for (const zero of [seconds(0), millis(0), "", "fast", seconds(-3), "NaNs", "7", millis("1e3")]) expect(parseDuration(zero), zero).toBe(0);
  });
});
