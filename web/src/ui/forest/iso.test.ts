import { describe, expect, it } from "vitest";
import { buildBoard } from "./board";
import { demoYear } from "./demoYear";
import { type Camera, TILE_CORNERS, TW, boardBounds, cellCenter, cellsBounds, clampCamera, fitCamera, hitTest, screenToWorld, worldToScreen, zoomAt } from "./iso";

const CASES = [
  { year: 2026, first: "monday" as const },
  { year: 2026, first: "sunday" as const },
  { year: 2024, first: "monday" as const },
  { year: 2012, first: "monday" as const }, // 54 columns
  { year: 2028, first: "sunday" as const }, // 54 columns
];

/** A fixed pseudo-random sequence: the same cameras on every run. */
function random(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1_664_525 + 1_013_904_223) % 4_294_967_296;
    return state / 4_294_967_296;
  };
}

function cameras(w: number, h: number, dims: { rows: number; cols: number }): Camera[] {
  const fit = fitCamera(boardBounds(dims), w, h, 36, 2.05 * TW);
  const next = random(7);
  const out: Camera[] = [fit, { s: 0.1, tx: 3, ty: 4 }, { s: 4.375, tx: -2000, ty: -900 }];
  for (let i = 0; i < 20; i++) out.push({ s: fit.s * 0.4 * 10 ** (next() * 1.8), tx: (next() - 0.5) * 3000, ty: (next() - 0.5) * 2000 });
  return out;
}

describe("hit-test by the inverse isometric transform", () => {
  for (const { year, first } of CASES) {
    it(`${year} ${first}: the centre of every day's cell hits that day under many cameras`, () => {
      const board = buildBoard(demoYear(year, { first }));
      for (const cam of cameras(1280, 800, board.dims)) {
        for (const day of board.days) {
          const hit = hitTest(cam, board.dims, worldToScreen(cam, cellCenter(day)));
          expect(hit, day.date).toEqual({ row: day.row, col: day.col });
        }
      }
    });

    it(`${year} ${first}: points just inside a cell's corners hit it, points just outside do not`, () => {
      const board = buildBoard(demoYear(year, { first }));
      const cam: Camera = { s: 1.3, tx: 500, ty: 120 };
      for (const cell of board.cells) {
        const c = cell.center;
        for (const corner of TILE_CORNERS) {
          const inside = worldToScreen(cam, { x: c.x + corner.x * 0.9, y: c.y + corner.y * 0.9 });
          expect(hitTest(cam, board.dims, inside)).toEqual({ row: cell.row, col: cell.col });
          const outside = worldToScreen(cam, { x: c.x + corner.x * 1.1, y: c.y + corner.y * 1.1 });
          expect(hitTest(cam, board.dims, outside)).not.toEqual({ row: cell.row, col: cell.col });
        }
      }
    });
  }

  it("outside the board, and a NaN or infinite point, is a miss", () => {
    const dims = { rows: 7, cols: 53 };
    const cam: Camera = { s: 1, tx: 0, ty: 0 };
    expect(hitTest(cam, dims, { x: -400, y: 10 })).toBeNull();
    expect(hitTest(cam, dims, { x: 0, y: -1 })).toBeNull();
    expect(hitTest(cam, dims, { x: 50_000, y: 50_000 })).toBeNull();
    for (const bad of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
      expect(hitTest(cam, dims, { x: bad, y: 3 })).toBeNull();
      expect(hitTest(cam, dims, { x: 3, y: bad })).toBeNull();
      expect(hitTest({ s: bad, tx: 0, ty: 0 }, dims, { x: 3, y: 3 })).toBeNull();
    }
    expect(hitTest({ s: 0, tx: 0, ty: 0 }, dims, { x: 3, y: 3 })).toBeNull();
  });
});

describe("camera", () => {
  it("screen and world are inverse", () => {
    const cam: Camera = { s: 2.5, tx: -40, ty: 17 };
    const p = screenToWorld(cam, worldToScreen(cam, { x: 12.5, y: -8 }));
    expect(p.x).toBeCloseTo(12.5);
    expect(p.y).toBeCloseTo(-8);
  });

  it("zoomAt keeps the point under the cursor fixed and respects the limits", () => {
    const cam: Camera = { s: 1, tx: 10, ty: 20 };
    const p = { x: 300, y: 200 };
    const before = screenToWorld(cam, p);
    const next = zoomAt(cam, p, 1.7, 0.1, 10);
    const after = screenToWorld(next, p);
    expect(after.x).toBeCloseTo(before.x);
    expect(after.y).toBeCloseTo(before.y);
    expect(zoomAt(cam, p, 1000, 0.1, 10).s).toBe(10);
    expect(zoomAt(cam, p, 0.0001, 0.1, 10).s).toBe(0.1);
  });

  it("fitCamera puts the whole board in the view, in a wide and a narrow window", () => {
    for (const [w, h] of [[1280, 360], [640, 360], [1280, 750]] as const) {
      const dims = { rows: 7, cols: 53 };
      const b = boardBounds(dims);
      const cam = fitCamera(b, w, h, 36, 2 * TW);
      const tl = worldToScreen(cam, { x: b.x0, y: b.y0 - 2 * TW });
      const br = worldToScreen(cam, { x: b.x1, y: b.y1 });
      expect(tl.x).toBeGreaterThanOrEqual(35);
      expect(tl.y).toBeGreaterThanOrEqual(35);
      expect(br.x).toBeLessThanOrEqual(w - 35);
      expect(br.y).toBeLessThanOrEqual(h - 35);
    }
  });

  it("clampCamera keeps part of the board in view however far a drag goes", () => {
    const box = boardBounds({ rows: 7, cols: 53 });
    for (const [tx, ty] of [[-1e6, 0], [1e6, 0], [0, -1e6], [0, 1e6], [Number.MAX_SAFE_INTEGER, -Number.MAX_SAFE_INTEGER]] as const) {
      const cam = clampCamera({ s: 1, tx, ty }, box, 800, 400, 120);
      expect(box.x1 * cam.s + cam.tx).toBeGreaterThanOrEqual(120 - 1e-6);
      expect(box.x0 * cam.s + cam.tx).toBeLessThanOrEqual(800 - 120 + 1e-6);
      expect(box.y1 * cam.s + cam.ty).toBeGreaterThanOrEqual(120 - 1e-6);
      expect(box.y0 * cam.s + cam.ty).toBeLessThanOrEqual(400 - 120 + 1e-6);
    }
  });

  it("the whole year lies along the width: a board of 53 weeks is much wider than it is tall", () => {
    const box = boardBounds({ rows: 7, cols: 53 });
    expect((box.x1 - box.x0) / (box.y1 - box.y0)).toBeGreaterThan(4);
    expect(TW).toBe(64);
  });

  it("cellsBounds covers the rhombi of the given cells and is null for none", () => {
    expect(cellsBounds([])).toBeNull();
    const box = cellsBounds([{ row: 0, col: 0 }, { row: 2, col: 5 }]);
    expect(box).not.toBeNull();
    const c = cellCenter({ row: 2, col: 5 });
    for (const corner of TILE_CORNERS) {
      expect(box?.x1).toBeGreaterThanOrEqual(c.x + corner.x);
      expect(box?.y1).toBeGreaterThanOrEqual(c.y + corner.y);
    }
    const first = cellCenter({ row: 0, col: 0 });
    for (const corner of TILE_CORNERS) expect(box?.y0).toBeLessThanOrEqual(first.y + corner.y);
    expect(box?.x1).toBeCloseTo(Math.max(...TILE_CORNERS.map((k) => c.x + k.x)));
  });
});
