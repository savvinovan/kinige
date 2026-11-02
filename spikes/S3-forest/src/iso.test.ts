import { describe, expect, it } from "vitest";
import { type FirstWeekday, type GridKind, allDays, dayPosition, gridDims, isoKey, prng } from "./calendar";
import { type Camera, TH, TW, boardBounds, cellCenter, fitCamera, hitTest, screenToWorld, worldToScreen, zoomAt } from "./iso";

const CASES: [GridKind, FirstWeekday, number][] = [
  ["A", "mon", 2026],
  ["A", "mon", 2024],
  ["B", "mon", 2026],
  ["B", "sun", 2026],
  ["B", "mon", 2024],
  ["B", "mon", 2012], // 54 columns
  ["B", "sun", 2028], // 54 columns
];

function cameras(w: number, h: number, dims: { rows: number; cols: number }): Camera[] {
  const fit = fitCamera(boardBounds(dims), w, h, 36, 1.6 * TW);
  const rnd = prng(7);
  const out: Camera[] = [fit, { s: 0.1, tx: 3, ty: 4 }, { s: 4.375, tx: -2000, ty: -900 }];
  for (let i = 0; i < 20; i++) out.push({ s: fit.s * 0.4 * 10 ** (rnd() * 1.8), tx: (rnd() - 0.5) * 3000, ty: (rnd() - 0.5) * 2000 });
  return out;
}

describe("hit-test by the inverse isometric transform", () => {
  for (const [grid, first, year] of CASES) {
    it(`${grid} ${first} ${year}: the centre of every day's cell hits that day, under many cameras`, () => {
      const dims = gridDims(grid, year, first);
      for (const cam of cameras(1280, 800, dims)) {
        for (const date of allDays(year)) {
          const pos = dayPosition(grid, date, first);
          expect(pos).not.toBeNull();
          if (!pos) continue;
          const hit = hitTest(cam, dims, worldToScreen(cam, cellCenter(pos)));
          expect(hit, isoKey(date)).toEqual(pos);
        }
      }
    });

    it(`${grid} ${first} ${year}: points just inside the rhombus corners hit the same cell, points just outside hit the neighbour or nothing`, () => {
      const dims = gridDims(grid, year, first);
      const cam: Camera = { s: 1.3, tx: 500, ty: 120 };
      for (let row = 0; row < dims.rows; row++) {
        for (let col = 0; col < dims.cols; col++) {
          const c = cellCenter({ row, col });
          for (const [dx, dy] of [[0.45, 0], [-0.45, 0], [0, 0.45], [0, -0.45]] as const) {
            const p = worldToScreen(cam, { x: c.x + dx * TW, y: c.y + dy * TH * 2 * 0.5 });
            expect(hitTest(cam, dims, p)).toEqual({ row, col });
          }
          const out = worldToScreen(cam, { x: c.x + 0.55 * TW, y: c.y });
          expect(hitTest(cam, dims, out)).not.toEqual({ row, col });
        }
      }
    });
  }

  it("outside the board is a miss", () => {
    const dims = gridDims("A", 2026, "mon");
    const cam: Camera = { s: 1, tx: 0, ty: 0 };
    expect(hitTest(cam, dims, { x: -40, y: 10 })).toBeNull();
    expect(hitTest(cam, dims, { x: 0, y: -1 })).toBeNull();
    expect(hitTest(cam, dims, { x: 5000, y: 5000 })).toBeNull();
    expect(hitTest(cam, dims, { x: Number.NaN, y: 3 })).toBeNull();
  });
});

describe("camera", () => {
  it("screen and world are inverse", () => {
    const cam: Camera = { s: 2.5, tx: -40, ty: 17 };
    const p = screenToWorld(cam, worldToScreen(cam, { x: 12.5, y: -8 }));
    expect(p.x).toBeCloseTo(12.5);
    expect(p.y).toBeCloseTo(-8);
  });

  it("zoomAt keeps the point under the cursor fixed", () => {
    const cam: Camera = { s: 1, tx: 10, ty: 20 };
    const p = { x: 300, y: 200 };
    const before = screenToWorld(cam, p);
    const next = zoomAt(cam, p, 1.7, 0.1, 10);
    const after = screenToWorld(next, p);
    expect(after.x).toBeCloseTo(before.x);
    expect(after.y).toBeCloseTo(before.y);
    expect(zoomAt(cam, p, 1000, 0.1, 10).s).toBe(10);
  });

  it("fitCamera puts the whole board in the view, for both grids and a narrow window", () => {
    for (const [w, h] of [[1280, 750], [640, 750]] as const) {
      for (const grid of ["A", "B"] as const) {
        const dims = gridDims(grid, 2026, "mon");
        const b = boardBounds(dims);
        const cam = fitCamera(b, w, h, 36, 1.6 * TW);
        const tl = worldToScreen(cam, { x: b.x0, y: b.y0 - 1.6 * TW });
        const br = worldToScreen(cam, { x: b.x1, y: b.y1 });
        expect(tl.x).toBeGreaterThanOrEqual(35);
        expect(tl.y).toBeGreaterThanOrEqual(35);
        expect(br.x).toBeLessThanOrEqual(w - 35);
        expect(br.y).toBeLessThanOrEqual(h - 35);
      }
    }
  });
});
