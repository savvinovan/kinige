import { describe, expect, it } from "vitest";
import {
  MAX_ZOOM,
  MIN_ZOOM,
  ORIGIN,
  ZOOM_STEPS,
  anchorOf,
  clampPan,
  clampZoom,
  fitScale,
  fittedSize,
  stepZoom,
  wheelZoomFactor,
  zoomAround,
  type Size,
} from "./viewerMath";
import { rect } from "../../test/rect";

const size = (width: number, height: number): Size => ({ width, height });
const STAGE = size(1000, 600);
const LANDSCAPE = size(1600, 900);
const GARBAGE = [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY, 1e308, -1e308, 0, -0];

const isFinitePoint = (point: { x: number; y: number }) => Number.isFinite(point.x) && Number.isFinite(point.y);

describe("clampZoom", () => {
  it("keeps the zoom between fit and the maximum", () => {
    expect(clampZoom(0.2)).toBe(MIN_ZOOM);
    expect(clampZoom(3)).toBe(3);
    expect(clampZoom(80)).toBe(MAX_ZOOM);
  });

  it("turns garbage into a usable zoom", () => {
    expect(clampZoom(Number.NaN)).toBe(MIN_ZOOM);
    expect(clampZoom(Number.POSITIVE_INFINITY)).toBe(MAX_ZOOM);
    expect(clampZoom(Number.NEGATIVE_INFINITY)).toBe(MIN_ZOOM);
  });
});

describe("stepZoom", () => {
  it("walks the ladder up and down and stops at the ends", () => {
    expect(stepZoom(1, 1)).toBe(1.5);
    expect(stepZoom(1.5, 1)).toBe(2);
    expect(stepZoom(2, -1)).toBe(1.5);
    expect(stepZoom(1, -1)).toBe(MIN_ZOOM);
    expect(stepZoom(MAX_ZOOM, 1)).toBe(MAX_ZOOM);
  });

  it("goes to the next step from a zoom between steps", () => {
    expect(stepZoom(1.7, 1)).toBe(2);
    expect(stepZoom(1.7, -1)).toBe(1.5);
  });

  it("never leaves the ladder bounds, whatever it is given", () => {
    for (const garbage of GARBAGE) {
      for (const direction of [-1, 1] as const) {
        const next = stepZoom(garbage, direction);
        expect(next).toBeGreaterThanOrEqual(MIN_ZOOM);
        expect(next).toBeLessThanOrEqual(MAX_ZOOM);
        expect(ZOOM_STEPS).toContain(next);
      }
    }
  });
});

describe("wheelZoomFactor", () => {
  it("zooms in on a negative delta and out on a positive one", () => {
    expect(wheelZoomFactor(-10, 0)).toBeGreaterThan(1);
    expect(wheelZoomFactor(10, 0)).toBeLessThan(1);
    expect(wheelZoomFactor(0, 0)).toBe(1);
  });

  it("does nothing for a delta that is not a number", () => {
    expect(wheelZoomFactor(Number.NaN, 0)).toBe(1);
  });

  it("caps a huge delta at one big notch", () => {
    const notch = wheelZoomFactor(-100, 0);
    expect(wheelZoomFactor(-1e308, 0)).toBe(notch);
    expect(wheelZoomFactor(Number.NEGATIVE_INFINITY, 0)).toBe(notch);
    expect(wheelZoomFactor(1e308, 2)).toBeCloseTo(1 / notch, 10);
  });

  it("gives a finite positive factor for any delta and delta mode", () => {
    for (const delta of GARBAGE) {
      for (const mode of [0, 1, 2, 7, Number.NaN]) {
        const factor = wheelZoomFactor(delta, mode);
        if (Number.isNaN(delta)) expect(factor).toBe(1);
        else {
          expect(Number.isFinite(factor)).toBe(true);
          expect(factor).toBeGreaterThan(0);
        }
      }
    }
  });

  it("reads lines and pages as more pixels", () => {
    expect(wheelZoomFactor(-1, 1)).toBeGreaterThan(wheelZoomFactor(-1, 0));
    expect(wheelZoomFactor(-1, 2)).toBeGreaterThan(wheelZoomFactor(-1, 1));
  });
});

describe("fitScale and fittedSize", () => {
  it("fits a landscape photo to the width and never enlarges a small one", () => {
    expect(fitScale(LANDSCAPE, STAGE, 0)).toBeCloseTo(0.625, 10);
    expect(fitScale(size(100, 50), STAGE, 0)).toBe(1);
  });

  it("fits a turned landscape photo to the stage height", () => {
    expect(fitScale(LANDSCAPE, STAGE, 1)).toBeCloseTo(0.375, 10);
    expect(fittedSize(LANDSCAPE, STAGE, 1)).toEqual(size(337.5, 600));
    // Half a turn is the same box as upright; the direction of the turns does not matter.
    expect(fitScale(LANDSCAPE, STAGE, 2)).toBeCloseTo(0.625, 10);
    expect(fitScale(LANDSCAPE, STAGE, 3)).toBeCloseTo(0.375, 10);
    expect(fitScale(LANDSCAPE, STAGE, -1)).toBeCloseTo(0.375, 10);
  });

  it("does not know a size it was not given", () => {
    expect(fitScale(null, STAGE, 0)).toBeNull();
    expect(fitScale(LANDSCAPE, size(0, 0), 0)).toBeNull();
    expect(fittedSize(null, STAGE, 1)).toBeNull();
  });

  it("refuses sizes and turns that are garbage", () => {
    for (const garbage of GARBAGE) {
      expect(fitScale(size(garbage, 100), STAGE, 0)).toSatisfy((value: number | null) => value === null || Number.isFinite(value));
      expect(fitScale(LANDSCAPE, size(garbage, garbage), 0)).toSatisfy((value: number | null) => value === null || Number.isFinite(value));
      const scale = fitScale(LANDSCAPE, STAGE, garbage);
      expect(scale === null || (Number.isFinite(scale) && scale > 0)).toBe(true);
    }
  });
});

describe("clampPan", () => {
  const fitted = size(800, 600);

  it("does not move a photo that fits", () => {
    expect(clampPan({ x: 50, y: -50 }, 1, fitted, STAGE)).toEqual({ x: 0, y: 0 });
  });

  it("keeps the edges of a zoomed photo outside the stage", () => {
    // 3x: 2400 by 1800 on a 1000 by 600 stage: the centre may move by half the overflow.
    expect(clampPan({ x: 9999, y: -9999 }, 3, fitted, STAGE)).toEqual({ x: 700, y: -600 });
    expect(clampPan({ x: 100, y: 100 }, 3, fitted, STAGE)).toEqual({ x: 100, y: 100 });
  });

  it("clamps each axis by its own overflow, and a small photo does not move at all", () => {
    // 2x: 1600 wide (overflows by 600), 1200 high (overflows by 600).
    expect(clampPan({ x: 400, y: 400 }, 2, fitted, STAGE)).toEqual({ x: 300, y: 300 });
    // A small photo at 2x still fits the stage: no pan.
    expect(clampPan({ x: 400, y: 400 }, 2, size(100, 100), STAGE)).toEqual({ x: 0, y: 0 });
  });

  it("gives a pan that is finite for any input", () => {
    for (const garbage of GARBAGE) {
      expect(isFinitePoint(clampPan({ x: garbage, y: garbage }, garbage, fitted, STAGE))).toBe(true);
      expect(isFinitePoint(clampPan({ x: 1, y: 1 }, 3, size(garbage, garbage), STAGE))).toBe(true);
      expect(isFinitePoint(clampPan({ x: 1, y: 1 }, 3, fitted, size(garbage, garbage)))).toBe(true);
    }
  });

  it("uses the stage as the photo when the photo size is not known", () => {
    expect(clampPan({ x: 5000, y: 0 }, 2, null, STAGE)).toEqual({ x: 500, y: 0 });
  });
});

describe("zoomAround", () => {
  const fitted = size(800, 600);

  it("keeps the point under the cursor in place", () => {
    const start = zoomAround({ zoom: 1, pan: ORIGIN }, 3, ORIGIN, fitted, STAGE);
    const anchor = { x: 200, y: -100 };
    const next = zoomAround(start, 4, anchor, fitted, STAGE);
    // The point of the photo under the anchor before: (anchor - pan) / zoom; after: the same.
    const before = { x: (anchor.x - start.pan.x) / start.zoom, y: (anchor.y - start.pan.y) / start.zoom };
    const after = { x: (anchor.x - next.pan.x) / next.zoom, y: (anchor.y - next.pan.y) / next.zoom };
    expect(after.x).toBeCloseTo(before.x, 8);
    expect(after.y).toBeCloseTo(before.y, 8);
  });

  it("returns to the centre at fit", () => {
    const zoomed = zoomAround({ zoom: 1, pan: ORIGIN }, 4, { x: 300, y: 100 }, fitted, STAGE);
    expect(zoomed.pan.x).not.toBe(0);
    expect(zoomAround(zoomed, 1, { x: 300, y: 100 }, fitted, STAGE)).toEqual({ zoom: 1, pan: { x: 0, y: 0 } });
  });

  it("stays between fit and the maximum", () => {
    expect(zoomAround({ zoom: 7, pan: ORIGIN }, 70, ORIGIN, fitted, STAGE).zoom).toBe(MAX_ZOOM);
    expect(zoomAround({ zoom: 2, pan: ORIGIN }, 0.01, ORIGIN, fitted, STAGE).zoom).toBe(MIN_ZOOM);
  });

  it("survives garbage in every argument", () => {
    for (const garbage of GARBAGE) {
      const result = zoomAround({ zoom: garbage, pan: { x: garbage, y: garbage } }, garbage, { x: garbage, y: garbage }, fitted, STAGE);
      expect(result.zoom).toBeGreaterThanOrEqual(MIN_ZOOM);
      expect(result.zoom).toBeLessThanOrEqual(MAX_ZOOM);
      expect(isFinitePoint(result.pan)).toBe(true);
    }
  });
});

describe("anchorOf", () => {
  it("is relative to the centre of the box", () => {
    expect(anchorOf(700, 400, rect(100, 100, 1000, 600))).toEqual({ x: 100, y: 0 });
  });

  it("is the centre for a pointer that is not a number", () => {
    expect(anchorOf(Number.NaN, Number.POSITIVE_INFINITY, rect(0, 0, 10, 10))).toEqual({ x: 0, y: 0 });
  });
});
