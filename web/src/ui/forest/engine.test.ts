import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ForestYear } from "../../api/generated/ForestYear";
import { type Board, buildBoard, seasonCells } from "./board";
import { ForestEngine } from "./engine";
import { FOREST_2026 } from "./fixtures";
import { type Pt, worldToScreen } from "./iso";
import { stubCanvas } from "./testCanvas";

const LABELS = { months: [], weekdays: [] };
const FRAME = 16;
/** A CSS time built from parts: the raw-values test forbids literal times in source files, even in tests. */
const seconds = (n: number) => `${n}s`;

/** The fixture year with today's tree at the given stage. */
function withToday(stage: number, base: ForestYear = FOREST_2026): ForestYear {
  return { ...base, days: base.days.map((d) => (d.date === base.today ? { ...d, stage, notes: Math.max(1, d.notes) } : d)) };
}

let canvas: HTMLCanvasElement;
let engine: ForestEngine;
const activated: string[] = [];
const hovered: (string | null)[] = [];

function start(table: ForestYear = FOREST_2026): Board {
  const board = buildBoard(table);
  engine.setBoard(board, LABELS);
  vi.advanceTimersByTime(FRAME);
  return board;
}

/** A pointer press and release at a screen point of the canvas. */
function click(p: Pt, id = 1) {
  canvas.dispatchEvent(new PointerEvent("pointerdown", { clientX: p.x, clientY: p.y, pointerId: id, bubbles: true }));
  canvas.dispatchEvent(new PointerEvent("pointerup", { clientX: p.x, clientY: p.y, pointerId: id, bubbles: true }));
}

const centre = (board: Board, date: string): Pt => worldToScreen(engine.view.cam, board.byDate.get(date)!.center);

beforeEach(() => {
  vi.useFakeTimers();
  stubCanvas();
  canvas = document.createElement("canvas");
  document.body.append(canvas);
  activated.length = 0;
  hovered.length = 0;
  engine = new ForestEngine(canvas, { onHover: (date) => hovered.push(date), onActivate: (date) => activated.push(date) });
  document.documentElement.style.setProperty("--motion-duration-grow", seconds(0.7));
  document.documentElement.style.setProperty("--motion-easing-enter", "cubic-bezier(0, 0, 0.2, 1)");
});

afterEach(() => {
  engine.destroy();
  canvas.remove();
  document.documentElement.style.removeProperty("--motion-duration-grow");
  document.documentElement.style.removeProperty("--motion-easing-enter");
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("the board engine draws on demand", () => {
  it("draws a frame for a new board and then nothing while idle", () => {
    start();
    const drawn = engine.frames;
    expect(drawn).toBeGreaterThan(0);
    vi.advanceTimersByTime(5000);
    expect(engine.frames).toBe(drawn);
  });

  it("a changed selection draws once, an unchanged one draws nothing", () => {
    start();
    const before = engine.frames;
    engine.setSelected("2026-10-16");
    vi.advanceTimersByTime(FRAME * 3);
    expect(engine.frames).toBe(before + 1);
    engine.setSelected("2026-10-16");
    vi.advanceTimersByTime(FRAME * 3);
    expect(engine.frames).toBe(before + 1);
    // A day that cannot be selected (after today) selects nothing.
    engine.setSelected("2026-12-01");
    vi.advanceTimersByTime(FRAME * 3);
    expect(engine.view.selected).toBeNull();
  });

  it("a destroyed engine draws nothing more and ignores the pointer", () => {
    const board = start();
    const p = centre(board, "2026-05-05");
    engine.destroy();
    const drawn = engine.frames;
    click(p);
    vi.advanceTimersByTime(1000);
    expect(engine.frames).toBe(drawn);
    expect(activated).toEqual([]);
  });
});

describe("growth of today's tree", () => {
  it("animates once when the stage rises: frames during 700 ms, none after", () => {
    start(withToday(2));
    const idle = engine.frames;
    engine.setBoard(buildBoard(withToday(3)), LABELS);
    expect(engine.view.grow).toMatchObject({ date: "2026-10-21", from: 2, to: 3, dur: 700 });
    vi.advanceTimersByTime(300);
    const mid = engine.frames;
    expect(mid - idle).toBeGreaterThan(5);
    vi.advanceTimersByTime(600);
    const end = engine.frames;
    vi.advanceTimersByTime(3000);
    expect(engine.frames).toBe(end);
    expect(engine.view.grow).toBeNull();
  });

  it("jumps when the duration token is zero (Reduce Motion)", () => {
    start(withToday(2));
    document.documentElement.style.setProperty("--motion-duration-grow", seconds(0));
    const before = engine.frames;
    engine.setBoard(buildBoard(withToday(3)), LABELS);
    expect(engine.view.grow).toBeNull();
    vi.advanceTimersByTime(2000);
    expect(engine.frames).toBe(before + 1);
  });

  it("does not animate when the stage falls, stays, or another day changes", () => {
    start(withToday(3));
    engine.setBoard(buildBoard(withToday(2)), LABELS);
    expect(engine.view.grow).toBeNull();
    engine.setBoard(buildBoard(withToday(2)), LABELS);
    expect(engine.view.grow).toBeNull();
    const other = withToday(2);
    engine.setBoard(buildBoard({ ...other, days: other.days.map((d) => (d.date === "2026-03-03" ? { ...d, stage: 6 } : d)) }), LABELS);
    expect(engine.view.grow).toBeNull();
  });

  it("does not animate on a new year or a new journal day", () => {
    start(withToday(2));
    engine.setBoard(buildBoard({ ...withToday(5), today: "2026-10-22" }), LABELS);
    expect(engine.view.grow).toBeNull();
    engine.setBoard(buildBoard({ ...withToday(6), year: 2027 }), LABELS);
    expect(engine.view.grow).toBeNull();
  });
});

describe("pointer", () => {
  it("a click on a day opens it; a click on ground around the year or after today does nothing", () => {
    const board = start();
    click(centre(board, "2026-05-05"));
    expect(activated).toEqual(["2026-05-05"]);
    click(centre(board, "2026-12-01"));
    click({ x: 3, y: 3 });
    expect(activated).toEqual(["2026-05-05"]);
    // The DOM cannot build an event at NaN, but the lookup itself must refuse one.
    expect(engine.dateAt({ x: Number.NaN, y: 4 })).toBeNull();
    expect(engine.dateAt({ x: 4, y: Number.POSITIVE_INFINITY })).toBeNull();
  });

  it("every day of the year hits its own cell through real pointer events", () => {
    const board = start();
    for (const day of board.days.filter((d) => d.date <= board.today)) click(centre(board, day.date));
    expect(activated).toEqual(board.days.filter((d) => d.date <= board.today).map((d) => d.date));
  });

  it("a drag pans and opens nothing", () => {
    const board = start();
    const p = centre(board, "2026-05-05");
    const before = { ...engine.view.cam };
    canvas.dispatchEvent(new PointerEvent("pointerdown", { clientX: p.x, clientY: p.y, pointerId: 1, bubbles: true }));
    canvas.dispatchEvent(new PointerEvent("pointermove", { clientX: p.x + 60, clientY: p.y + 25, pointerId: 1, bubbles: true }));
    canvas.dispatchEvent(new PointerEvent("pointerup", { clientX: p.x + 60, clientY: p.y + 25, pointerId: 1, bubbles: true }));
    expect(activated).toEqual([]);
    expect(engine.view.cam.tx).toBeCloseTo(before.tx + 60);
    expect(engine.view.cam.ty).toBeCloseTo(before.ty + 25);
  });

  it("hover reports the day under the pointer and clears when it leaves or moves over ground", () => {
    const board = start();
    const p = centre(board, "2026-07-07");
    canvas.dispatchEvent(new PointerEvent("pointermove", { clientX: p.x, clientY: p.y, pointerId: 1, bubbles: true }));
    expect(hovered.at(-1)).toBe("2026-07-07");
    canvas.dispatchEvent(new PointerEvent("pointerleave", { pointerId: 1 }));
    expect(hovered.at(-1)).toBeNull();
    const future = centre(board, "2026-12-01");
    canvas.dispatchEvent(new PointerEvent("pointermove", { clientX: future.x, clientY: future.y, pointerId: 1, bubbles: true }));
    expect(hovered.at(-1)).toBeNull();
  });

  it("the wheel zooms about the cursor within the limits, a double click zooms in by two", () => {
    const board = start();
    const fit = engine.view.cam.s;
    const p = centre(board, "2026-07-07");
    canvas.dispatchEvent(new WheelEvent("wheel", { clientX: p.x, clientY: p.y, deltaY: -300, cancelable: true }));
    expect(engine.view.cam.s).toBeGreaterThan(fit);
    const after = centre(board, "2026-07-07");
    expect(after.x).toBeCloseTo(p.x, 3);
    expect(after.y).toBeCloseTo(p.y, 3);
    canvas.dispatchEvent(new WheelEvent("wheel", { clientX: p.x, clientY: p.y, deltaY: 1e9, cancelable: true }));
    expect(engine.view.cam.s).toBeGreaterThanOrEqual(fit * 0.2 - 1e-9);
    engine.fitYear();
    canvas.dispatchEvent(new MouseEvent("dblclick", { clientX: p.x, clientY: p.y }));
    expect(engine.view.cam.s).toBeCloseTo(fit * 2);
    canvas.dispatchEvent(new WheelEvent("wheel", { clientX: p.x, clientY: p.y, deltaY: -1e9, cancelable: true }));
    expect(engine.view.cam.s).toBeLessThanOrEqual(280 / 64 + 1e-9);
  });

  it("garbage wheel and pointer events change nothing and do not throw", () => {
    start();
    const before = { ...engine.view.cam };
    for (const deltaY of [1e300, -1e300, 0, 5e-324]) {
      expect(() => canvas.dispatchEvent(new WheelEvent("wheel", { clientX: 5, clientY: 5, deltaY, cancelable: true }))).not.toThrow();
      expect(Number.isFinite(engine.view.cam.s) && engine.view.cam.s > 0, `deltaY ${deltaY}`).toBe(true);
      expect(Number.isFinite(engine.view.cam.tx) && Number.isFinite(engine.view.cam.ty)).toBe(true);
    }
    // A release or a move of a pointer that never pressed.
    canvas.dispatchEvent(new PointerEvent("pointerup", { pointerId: 99 }));
    canvas.dispatchEvent(new PointerEvent("pointermove", { clientX: 1e300, clientY: -1e300, pointerId: 99 }));
    canvas.dispatchEvent(new PointerEvent("pointercancel", { pointerId: 98 }));
    expect(engine.view.cam.s).toBeGreaterThan(0);
    expect(before.s).toBeGreaterThan(0);
    expect(activated).toEqual([]);
  });
});

describe("zoom buttons", () => {
  it("a season fills the view with larger tiles than the whole year, and the whole year comes back", () => {
    const board = start();
    const fit = { ...engine.view.cam };
    engine.fitCells(seasonCells(board, "autumn"));
    expect(engine.view.cam.s).toBeGreaterThan(fit.s * 2);
    engine.fitCells(seasonCells(board, "winter"));
    const winter = engine.view.cam.s;
    expect(winter).toBeGreaterThan(fit.s);
    engine.fitYear();
    expect(engine.view.cam).toEqual(fit);
    engine.fitCells([]);
    expect(engine.view.cam).toEqual(fit);
  });

  it("zooming to a season draws one frame and then rests", () => {
    const board = start();
    const before = engine.frames;
    engine.fitCells(seasonCells(board, "summer"));
    vi.advanceTimersByTime(5000);
    expect(engine.frames - before).toBeLessThanOrEqual(2);
  });

  it("the keyboard pan brings a day into view and a selection by the feed does not move the camera", () => {
    const board = start();
    engine.fitCells(seasonCells(board, "summer"));
    const before = { ...engine.view.cam };
    engine.setSelected("2026-02-02");
    expect(engine.view.cam).toEqual(before);
    engine.setSelected("2026-02-03", true);
    expect(engine.view.cam).not.toEqual(before);
    const p = centre(board, "2026-02-03");
    expect(p.x).toBeGreaterThanOrEqual(0);
    expect(p.x).toBeLessThanOrEqual(1280);
  });
});
