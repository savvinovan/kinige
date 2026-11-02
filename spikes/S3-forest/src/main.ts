import "./style.css";
import { allDays, isoKey, stageHistogram } from "./calendar";
import { type BenchOptions, runBench, webglInfo } from "./bench";
import { Forest } from "./forest";

const el = <T extends HTMLElement>(id: string): T => {
  const e = document.getElementById(id);
  if (!e) throw new Error(`#${id} missing`);
  return e as T;
};

const forest = new Forest(el<HTMLCanvasElement>("forest"), el("card"), el("live"), el("info"));

const gridButtons = document.querySelectorAll<HTMLButtonElement>("[data-grid]");
const firstButtons = document.querySelectorAll<HTMLButtonElement>("[data-first]");
const growButton = el<HTMLButtonElement>("grow");

function syncButtons(): void {
  for (const b of gridButtons) b.setAttribute("aria-pressed", String(b.dataset.grid === forest.gridKind));
  for (const b of firstButtons) {
    b.setAttribute("aria-pressed", String(b.dataset.first === forest.firstWeekday));
    b.disabled = forest.gridKind === "A";
  }
}

for (const b of gridButtons) {
  b.addEventListener("click", () => {
    forest.setGrid(b.dataset.grid === "B" ? "B" : "A");
    syncButtons();
  });
}
for (const b of firstButtons) {
  b.addEventListener("click", () => {
    forest.setFirstWeekday(b.dataset.first === "sun" ? "sun" : "mon");
    syncButtons();
  });
}
el("fit").addEventListener("click", () => forest.fitYear(true));
growButton.addEventListener("click", () => {
  forest.addNoteToday();
  forest.canvas.focus();
});

// Test and benchmark hooks (this is a spike page: the Playwright scripts drive it through window.__forest).
const api = {
  setGrid: (g: "A" | "B") => { forest.setGrid(g); syncButtons(); },
  setFirstWeekday: (f: "mon" | "sun") => { forest.setFirstWeekday(f); syncButtons(); },
  fitYear: () => forest.fitYear(false),
  zoomToDate: (key: string, tilePx: number) => forest.zoomToDate(key, tilePx),
  select: (key: string | null) => forest.select(key),
  hover: (key: string | null) => forest.hover(key),
  addNoteToday: () => forest.addNoteToday(),
  days: () => allDays(2026).map(isoKey),
  screenOf: (key: string) => {
    const p = forest.screenOf(key);
    const r = forest.canvas.getBoundingClientRect();
    return p ? { x: p.x + r.left, y: p.y + r.top } : null;
  },
  dateAt: (x: number, y: number) => {
    const r = forest.canvas.getBoundingClientRect();
    return forest.dateAt({ x: x - r.left, y: y - r.top });
  },
  selected: () => {
    const s = forest.view.selected;
    const c = s && forest.scene.cells[s.row * forest.scene.dims.cols + s.col];
    return c?.key ?? null;
  },
  state: () => ({
    grid: forest.gridKind,
    dims: forest.scene.dims,
    cam: forest.view.cam,
    tilePx: 64 * forest.view.cam.s,
    viewport: { w: forest.w, h: forest.h },
    dpr: forest.dpr,
    stages: stageHistogram(forest.scene.data),
    growing: forest.view.grow !== null,
    frames: forest.frames,
    todayStage: forest.scene.data.get("2026-10-16")?.stage ?? -1,
  }),
  bench: (opts?: BenchOptions) => runBench(forest, opts),
  webgl: webglInfo,
  limits: () => forest.limits,
  setAdaptive: (on: boolean) => { forest.adaptive = on; },
  setMaxDpr: (n: number) => forest.setMaxDpr(n),
  setSprites: (on: boolean) => forest.setSprites(on),
  pickTransparencyDemo: () => forest.pickTransparencyDemo(),
};
(window as unknown as { __forest: typeof api }).__forest = api;

// Optional start state from the URL, for screenshots: ?grid=B&first=sun
const q = new URLSearchParams(location.search);
if (q.get("grid") === "B") api.setGrid("B");
if (q.get("first") === "sun") api.setFirstWeekday("sun");
if (q.get("sprites") === "1") api.setSprites(true);
syncButtons();
forest.fitYear(false);
// The canvas text needs the web font; redraw once it is there.
void document.fonts.ready.then(() => forest.invalidate());
