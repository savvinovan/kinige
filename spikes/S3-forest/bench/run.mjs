// Measurements and screenshots of the S3 page in real browsers (Playwright). Usage:
//   node bench/run.mjs --config chromium|chromium-nogpu|chromium-swiftshader|webkit [--out results] [--shots ../../docs/screens/S3] [--seconds 3]
// Headless FPS is an approximation of a real window: the numbers are about the renderer, not about a display's refresh.
import { chromium, webkit } from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import os from "node:os";
import { serve } from "./serve.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const argv = Object.fromEntries(
  process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith("--") ? [...acc, [a.slice(2), all[i + 1] && !all[i + 1].startsWith("--") ? all[i + 1] : "true"]] : acc), []),
);

const CONFIGS = {
  chromium: { type: chromium, args: [] },
  "chromium-nogpu": { type: chromium, args: ["--disable-gpu"] },
  "chromium-swiftshader": { type: chromium, args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] },
  webkit: { type: webkit, args: [] },
};
const config = argv.config ?? "chromium";
const cfg = CONFIGS[config];
if (!cfg) throw new Error(`unknown config ${config}`);
const seconds = Number(argv.seconds ?? 3);
const outDir = resolve(here, "..", argv.out ?? "results");
const shotsDir = argv.shots ? resolve(here, "..", argv.shots) : null;
// `--shots-only year`: just the whole-year pictures (for the second browser).
const skipPerf = argv["skip-perf"] === "true"; // local iteration on the checks below
// `--variants wide,wide-cached`: only these window variants (the slow software-WebGL configuration cannot run them all).
const only = argv.variants ? new Set(argv.variants.split(",")) : null;
const perfOnly = argv["perf-only"] === "true"; // skip the click-through and motion checks (local iteration)
const yearOnly = argv["shots-only"] === "year";

// Without `cache`, every frame redraws the whole board (the plain Canvas 2D path); with it, pan and zoom blit a layer.
// "xN": the scene is drawn N times per frame, to find the headroom under the 16.7 ms budget.
const VIEWPORTS = [
  { name: "wide", width: 1280, height: 800, dpr: 1, load: 1 },
  { name: "retina", width: 1280, height: 800, dpr: 2, load: 1 },
  { name: "narrow", width: 640, height: 800, dpr: 1, load: 1 },
  { name: "retina-x2", width: 1280, height: 800, dpr: 2, load: 2 },
  { name: "retina-x4", width: 1280, height: 800, dpr: 2, load: 4 },
  // Trees drawn from bitmaps: the cost model of the real art pipeline (atlas frames).
  { name: "retina-sprites", width: 1280, height: 800, dpr: 2, load: 1, sprites: true },
  { name: "retina-x4-sprites", width: 1280, height: 800, dpr: 2, load: 4, sprites: true },
  // The canvas drawn at density 1 (what lowering the density during a gesture would give).
  { name: "retina-lowres", width: 1280, height: 800, dpr: 2, load: 1, maxDpr: 1 },
  // The floor of the browser: only a full-canvas fill per frame, no trees.
  { name: "retina-clear-only", width: 1280, height: 800, dpr: 2, load: 1, clearOnly: true },
  // The layer cache.
  { name: "wide-cached", width: 1280, height: 800, dpr: 1, load: 1, cache: true },
  { name: "retina-cached", width: 1280, height: 800, dpr: 2, load: 1, cache: true },
  { name: "narrow-cached", width: 640, height: 800, dpr: 1, load: 1, cache: true },
  { name: "retina-x4-cached", width: 1280, height: 800, dpr: 2, load: 4, cache: true },
  { name: "retina-x8-cached", width: 1280, height: 800, dpr: 2, load: 8, cache: true },
];

const server = await serve(resolve(here, "..", "dist"));
const browser = await cfg.type.launch({ args: cfg.args });
const result = {
  config,
  os: `${os.platform()} ${os.release()} ${os.arch()}`,
  cpus: `${os.cpus().length} x ${os.cpus()[0]?.model ?? "?"}`,
  browser: `${config.startsWith("chromium") ? "chromium" : "webkit"} ${browser.version()}`,
  seconds,
  runs: [],
};

async function open(vp, opts = {}) {
  const ctx = await browser.newContext({
    viewport: { width: vp.width, height: vp.height },
    deviceScaleFactor: vp.dpr,
    reducedMotion: opts.reducedMotion ?? "no-preference",
  });
  const page = await ctx.newPage();
  page.on("pageerror", (e) => console.error("pageerror:", e.message));
  await page.goto(server.url + (opts.query ?? ""));
  await page.waitForFunction(() => window.__forest !== undefined);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(300);
  return { ctx, page };
}

// ---- 1. FPS on scripted pan / zoom ----------------------------------------------------------------------------------
{
  const first = await open(VIEWPORTS[0]);
  result.webgl = await first.page.evaluate(() => window.__forest.webgl());
  await first.ctx.close();
}
for (const vp of skipPerf ? [] : VIEWPORTS.filter((v) => !only || only.has(v.name))) {
  for (const grid of ["A", "B"]) {
    const { ctx, page } = await open(vp);
    await page.evaluate(([g, sp, md, ch]) => { window.__forest.setCache(ch); window.__forest.setGrid(g); window.__forest.setSprites(sp); window.__forest.setMaxDpr(md); }, [grid, vp.sprites === true, vp.maxDpr ?? Infinity, vp.cache === true]);
    await page.waitForTimeout(200);
    const state = await page.evaluate(() => window.__forest.state());
    const phases = await page.evaluate(([s, l, c]) => window.__forest.bench({ seconds: s, loadFactor: l, clearOnly: c }), [seconds, vp.load, vp.clearOnly === true]);
    result.runs.push({ viewport: vp, grid, tilePxAtFit: state.tilePx, canvas: state.viewport, phases });
    console.log(`${config} ${vp.name} x${vp.load} grid ${grid}: ` + phases.map((p) => `${p.name} ${p.fps.toFixed(1)} fps p95 ${p.p95Ms.toFixed(1)} ms draw ${p.drawMeanMs.toFixed(2)} ms [${p.lods.join("/")}]`).join(" | "));
    await ctx.close();
  }
}

// ---- 2. hit-test through real pointer events: every day of the year, both grids ------------------------------------------
async function hitTestAll(page, label) {
  return page.evaluate(() => 0).then(async () => {
    const days = await page.evaluate(() => window.__forest.days());
    const misses = [];
    let clicks = 0;
    for (const zoom of [null, 110]) {
      await page.evaluate((z) => { window.__forest.select(null); window.__forest.fitYear(); void z; }, zoom);
      for (const key of days) {
        if (zoom) await page.evaluate(([k, z]) => window.__forest.zoomToDate(k, z), [key, zoom]);
        const p = await page.evaluate((k) => window.__forest.screenOf(k), key);
        if (!p) { misses.push(`${key}: no position`); continue; }
        await page.mouse.click(p.x, p.y);
        clicks++;
        const got = await page.evaluate(() => window.__forest.selected());
        if (got !== key) misses.push(`${key} -> ${got}`);
      }
    }
    return { label, clicks, misses: misses.length, first: misses.slice(0, 5) };
  });
}
result.hit = [];
for (const vp of perfOnly ? [] : [VIEWPORTS[0], VIEWPORTS[1]]) {
  const { ctx, page } = await open(vp, { reducedMotion: "reduce" });
  for (const [grid, first] of [["A", "mon"], ["B", "mon"], ["B", "sun"]]) {
    await page.evaluate(([g, f]) => { window.__forest.setGrid(g); window.__forest.setFirstWeekday(f); }, [grid, first]);
    const r = await hitTestAll(page, `${grid}-${first} ${vp.name}`);
    result.hit.push(r);
    console.log(`${config} hit-test ${r.label}: ${r.clicks} clicks, ${r.misses} misses`, r.first);
  }
  await ctx.close();
}

// ---- 3. no gratuitous motion and the growth animation -----------------------------------------------------------------
result.motion = { idleFramesIn1500ms: -1, stageBefore: -1, stageAfter: -1, reducedMotionAnimates: false };
if (!perfOnly) {
  const { ctx, page } = await open(VIEWPORTS[0]);
  const f0 = (await page.evaluate(() => window.__forest.state())).frames;
  await page.waitForTimeout(1500);
  const f1 = (await page.evaluate(() => window.__forest.state())).frames;
  const before = await page.evaluate(() => window.__forest.state().todayStage);
  const grew = await page.evaluate(() => window.__forest.addNoteToday());
  const during = await page.evaluate(() => window.__forest.state().growing);
  await page.waitForTimeout(1200);
  const after = await page.evaluate(() => window.__forest.state());
  const f2 = after.frames;
  await page.waitForTimeout(800);
  const f3 = (await page.evaluate(() => window.__forest.state())).frames;
  // A second note must not animate when the stage does not change: add notes until the stage stops growing.
  let extra = 0;
  while (await page.evaluate(() => window.__forest.addNoteToday())) extra++;
  await page.waitForTimeout(1000);
  result.motion = {
    idleFramesIn1500ms: f1 - f0,
    stageBefore: before,
    stageAfter: after.todayStage,
    addNoteGrew: grew,
    animatingRightAfter: during,
    animatingAfter1200ms: after.growing,
    framesWhileAnimating: f2 - f1,
    framesAfterSettled800ms: f3 - f2,
    notesUntilMaxStage: extra,
  };
  console.log(config, "motion", result.motion);
  await ctx.close();
  const r = await open(VIEWPORTS[0], { reducedMotion: "reduce" });
  await r.page.evaluate(() => window.__forest.addNoteToday());
  result.motion.reducedMotionAnimates = await r.page.evaluate(() => window.__forest.state().growing);
  await r.ctx.close();
}

// ---- 3b. gestures and keys through real input events --------------------------------------------------------------------
result.gestures = {};
if (!perfOnly) {
  const { ctx, page } = await open(VIEWPORTS[0], { reducedMotion: "reduce" });
  const st = () => page.evaluate(() => window.__forest.state());
  const check = (name, ok) => { result.gestures[name] = ok; if (!ok) console.error(`${config} gesture FAILED: ${name}`); };
  const cx = 640;
  const cy = 420;
  let a = await st();
  await page.mouse.move(cx, cy);
  await page.mouse.wheel(0, -300);
  await page.waitForTimeout(100);
  let b = await st();
  check("wheel-up zooms in", b.cam.s > a.cam.s);
  await page.mouse.wheel(0, 300);
  await page.waitForTimeout(100);
  const c = await st();
  check("wheel-down zooms out again", c.cam.s < b.cam.s);
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -200);
  await page.keyboard.up("Control");
  await page.waitForTimeout(100);
  const d = await st();
  check("ctrl+wheel (Chromium pinch) zooms in", d.cam.s > c.cam.s);
  await page.evaluate(() => window.__forest.fitYear());
  a = await st();
  await page.mouse.dblclick(cx, cy);
  await page.waitForTimeout(100);
  b = await st();
  check("double click zooms in about 2x", b.cam.s > a.cam.s * 1.8 && b.cam.s < a.cam.s * 2.2);
  await page.evaluate(() => window.__forest.select(null)); // the double click above selected a day
  await page.mouse.move(cx, cy);
  await page.mouse.down();
  await page.mouse.move(cx + 120, cy + 60, { steps: 6 });
  await page.mouse.up();
  await page.waitForTimeout(100);
  const e = await st();
  check("drag pans by the drag distance", Math.abs(e.cam.tx - b.cam.tx - 120) < 2 && Math.abs(e.cam.ty - b.cam.ty - 60) < 2);
  check("a drag does not select a day", (await page.evaluate(() => window.__forest.selected())) === null);
  await page.click("#fit");
  await page.waitForTimeout(100);
  const f = await st();
  check("Whole year button restores the fit", Math.abs(f.cam.s - a.cam.s) < 1e-6);
  // Keys: arrows move between days, Escape clears.
  for (const [grid, first, down] of [["A", "mon", "2026-02-28"], ["B", "mon", "2026-02-07"]]) {
    await page.evaluate(([g, fw]) => { window.__forest.setGrid(g); window.__forest.setFirstWeekday(fw); window.__forest.select("2026-01-31"); }, [grid, first]);
    await page.focus("#forest");
    const pressed = async (key) => { await page.keyboard.press(key); return page.evaluate(() => window.__forest.selected()); };
    check(`${grid}: ArrowRight is the next day, across the month`, (await pressed("ArrowRight")) === "2026-02-01");
    check(`${grid}: ArrowLeft is the previous day`, (await pressed("ArrowLeft")) === "2026-01-31");
    check(`${grid}: ArrowDown is ${grid === "A" ? "the next month (clamped)" : "the next week"}`, (await pressed("ArrowDown")) === down);
    check(`${grid}: Escape clears the selection`, (await pressed("Escape")) === null);
  }
  await page.evaluate(() => { window.__forest.select("2026-12-31"); });
  await page.focus("#forest");
  await page.keyboard.press("ArrowRight");
  check("ArrowRight at the end of the year stays", (await page.evaluate(() => window.__forest.selected())) === "2026-12-31");
  console.log(config, "gestures", result.gestures);
  await ctx.close();
}

// ---- 4. screenshots ------------------------------------------------------------------------------------------------------
if (shotsDir) {
  await mkdir(shotsDir, { recursive: true });
  const shot = async (page, name) => page.screenshot({ path: join(shotsDir, `${name}.png`) });
  const wide = VIEWPORTS[0];
  const narrow = VIEWPORTS.find((v) => v.name === "narrow");
  for (const grid of ["A", "B"]) {
    const g = grid.toLowerCase();
    // Whole year, clean and with a selection plus a hover card.
    const { ctx, page } = await open(wide, { reducedMotion: "reduce" });
    await page.evaluate((x) => window.__forest.setGrid(x), grid);
    await page.waitForTimeout(150);
    await shot(page, `${g}-year-1280x800`);
    await page.evaluate(() => { window.__forest.select("2026-10-16"); window.__forest.hover("2026-07-04"); });
    await page.waitForTimeout(150);
    await shot(page, `${g}-year-selected-hover-1280x800`);
    if (yearOnly) {
      await ctx.close();
      const n = await open(narrow, { reducedMotion: "reduce" });
      await n.page.evaluate((x) => window.__forest.setGrid(x), grid);
      await n.page.waitForTimeout(150);
      await shot(n.page, `${g}-year-640x800`);
      await n.ctx.close();
      continue;
    }
    // Zoomed in on a tree with tall trees in front of it.
    await page.evaluate(() => { window.__forest.hover(null); });
    const demo = await page.evaluate(() => window.__forest.pickTransparencyDemo());
    await page.evaluate((k) => { window.__forest.zoomToDate(k, 120); window.__forest.select(k); }, demo);
    await page.waitForTimeout(150);
    await shot(page, `${g}-zoom-selected-1280x800`);
    // Levels of detail on the same board: dot, simple, detailed.
    for (const [lod, px] of [["dot", 9], ["simple", 22], ["detailed", 70]]) {
      await page.evaluate(([k, p]) => { window.__forest.select(null); window.__forest.zoomToDate(k, p); }, ["2026-07-01", px]);
      await page.waitForTimeout(150);
      await shot(page, `${g}-lod-${lod}-1280x800`);
    }
    // Growth of today's tree, before and after (reduced motion: the frame after is final).
    await page.evaluate(() => { window.__forest.fitYear(); window.__forest.zoomToDate("2026-10-16", 140); window.__forest.select("2026-10-16"); });
    await page.waitForTimeout(150);
    await shot(page, `${g}-growth-before-1280x800`);
    await page.evaluate(() => window.__forest.addNoteToday());
    await page.waitForTimeout(150);
    await shot(page, `${g}-growth-after-1280x800`);
    await ctx.close();
    // Narrow window.
    const n = await open(narrow, { reducedMotion: "reduce" });
    await n.page.evaluate((x) => window.__forest.setGrid(x), grid);
    await n.page.waitForTimeout(150);
    await shot(n.page, `${g}-year-640x800`);
    await n.page.evaluate(() => { window.__forest.select("2026-10-16"); window.__forest.hover("2026-07-04"); });
    await n.page.waitForTimeout(150);
    await shot(n.page, `${g}-year-selected-hover-640x800`);
    await n.ctx.close();
  }
}

await mkdir(outDir, { recursive: true });
await writeFile(join(outDir, `${process.env.RESULT_PREFIX ?? os.platform()}-${config}.json`), JSON.stringify(result, null, 2));
await browser.close();
await server.close();
