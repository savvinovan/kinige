// In-page benchmark: a scripted camera (pan, zoom, both) drives the real renderer frame by frame and records the
// rAF intervals and the time of the draw call. Headless FPS is an approximation of a real window; see the README.

import type { Forest } from "./forest";

export interface PhaseResult {
  name: string;
  frames: number;
  seconds: number;
  fps: number;
  p50Ms: number;
  p95Ms: number;
  p99Ms: number;
  maxMs: number;
  /** Frames whose interval exceeded 20 ms (a missed 60 Hz frame). */
  slowFrames: number;
  drawMeanMs: number;
  drawP95Ms: number;
  lods: string[];
}

export interface BenchOptions {
  seconds?: number;
  /** Draw the scene this many times per frame: finds how much headroom is left under the 16.7 ms budget. */
  loadFactor?: number;
  /** Only clear the canvas each frame: the floor of the browser (a full-canvas fill) without any trees. */
  clearOnly?: boolean;
}

function pct(sorted: number[], q: number): number {
  if (sorted.length === 0) return 0;
  return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))] ?? 0;
}

export async function runBench(f: Forest, opts: BenchOptions = {}): Promise<PhaseResult[]> {
  const seconds = opts.seconds ?? 3;
  const load = Math.max(1, opts.loadFactor ?? 1);
  const clearOnly = opts.clearOnly === true;
  const out: PhaseResult[] = [];
  f.frozen = true;
  f.fitYear(false);
  const fit = f.view.cam;
  const { minScale, maxScale } = f.limits;
  const cx = f.w / 2;
  const cy = f.h / 2;
  // World point at the middle of the board, kept under the screen centre while zooming.
  const phases: [string, (t: number) => { s: number; tx: number; ty: number }][] = [
    ["pan", (t) => {
      const s = fit.s * 1.6;
      const r = 0.22 * Math.min(f.w, f.h);
      const a = (2 * Math.PI * t) / 3;
      return { s, tx: cx - (cx - fit.tx) * 1.6 + r * Math.cos(a), ty: cy - (cy - fit.ty) * 1.6 + r * Math.sin(a) };
    }],
    ["zoom", (t) => {
      const lo = Math.log(minScale);
      const hi = Math.log(maxScale * 0.6);
      const s = Math.exp(lo + ((hi - lo) * (1 - Math.cos((2 * Math.PI * t) / 4))) / 2);
      const k = s / fit.s;
      return { s, tx: cx - (cx - fit.tx) * k, ty: cy - (cy - fit.ty) * k };
    }],
    ["pan+zoom", (t) => {
      const lo = Math.log(fit.s * 0.8);
      const hi = Math.log(fit.s * 4);
      const s = Math.exp(lo + ((hi - lo) * (1 - Math.cos((2 * Math.PI * t) / 5))) / 2);
      const k = s / fit.s;
      const r = 0.2 * Math.min(f.w, f.h);
      const a = (2 * Math.PI * t) / 2.5;
      return { s, tx: cx - (cx - fit.tx) * k + r * Math.cos(a), ty: cy - (cy - fit.ty) * k + r * Math.sin(a) };
    }],
  ];
  // A short warm-up (JIT, font, sprite baking) that is not reported.
  const warm = phases[0];
  if (warm) await runPhase(f, "warm-up", warm[1], 0.5, 1, false);
  for (const [name, camAt] of phases) {
    out.push(await runPhase(f, name, camAt, seconds, load, clearOnly));
  }
  f.frozen = false;
  f.fitYear(false);
  return out;
}

function runPhase(
  f: Forest,
  name: string,
  camAt: (t: number) => { s: number; tx: number; ty: number },
  seconds: number,
  load: number,
  clearOnly: boolean,
): Promise<PhaseResult> {
  return new Promise((resolve) => {
    const intervals: number[] = [];
    const draws: number[] = [];
    const lods = new Set<string>();
    let t0 = 0;
    let last = 0;
    const tick = (now: number) => {
      if (t0 === 0) t0 = now;
      const t = (now - t0) / 1000;
      if (last !== 0) intervals.push(now - last);
      last = now;
      f.view.cam = camAt(t);
      const a = performance.now();
      if (clearOnly) {
        f.ctx.fillStyle = "#141414";
        f.ctx.fillRect(0, 0, f.w, f.h);
      } else {
        for (let i = 0; i < load; i++) f.frame(now);
      }
      draws.push(performance.now() - a);
      const w = 64 * f.view.cam.s;
      lods.add(w < 14 ? "dot" : w < 32 ? "simple" : "detailed");
      if (t < seconds) {
        requestAnimationFrame(tick);
        return;
      }
      const sorted = [...intervals].sort((x, y) => x - y);
      const dsorted = [...draws].sort((x, y) => x - y);
      const total = intervals.reduce((x, y) => x + y, 0);
      resolve({
        name,
        frames: intervals.length,
        seconds: total / 1000,
        fps: total > 0 ? (intervals.length * 1000) / total : 0,
        p50Ms: pct(sorted, 0.5),
        p95Ms: pct(sorted, 0.95),
        p99Ms: pct(sorted, 0.99),
        maxMs: sorted[sorted.length - 1] ?? 0,
        slowFrames: intervals.filter((v) => v > 20).length,
        drawMeanMs: draws.reduce((x, y) => x + y, 0) / Math.max(1, draws.length),
        drawP95Ms: pct(dsorted, 0.95),
        lods: [...lods],
      });
    };
    requestAnimationFrame(tick);
  });
}

/** What kind of WebGL the browser would give PixiJS: the renderer string tells software (SwiftShader, llvmpipe) from a GPU. */
export function webglInfo(): { ok: boolean; renderer: string; version: string } {
  const c = document.createElement("canvas");
  const gl = c.getContext("webgl2") ?? c.getContext("webgl");
  if (!gl) return { ok: false, renderer: "", version: "" };
  const ext = gl.getExtension("WEBGL_debug_renderer_info");
  const renderer = ext ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)) : String(gl.getParameter(gl.RENDERER));
  return { ok: true, renderer, version: String(gl.getParameter(gl.VERSION)) };
}
