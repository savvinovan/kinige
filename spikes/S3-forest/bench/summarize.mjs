// Turns results/*.json into one Markdown table (results/summary.md) for the README.
import { readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const dir = resolve(dirname(fileURLToPath(import.meta.url)), "..", process.argv[2] ?? "results");
const files = (await readdir(dir)).filter((f) => f.endsWith(".json")).sort();
const rows = [];
const hit = [];
const misc = [];
const gest = [];
for (const f of files) {
  const r = JSON.parse(await readFile(join(dir, f), "utf8"));
  const name = f.replace(/\.json$/, "");
  for (const run of r.runs) {
    const ph = Object.fromEntries(run.phases.map((p) => [p.name, p]));
    const fmt = (p) => (p ? `${p.fps.toFixed(1)} / ${p.p95Ms.toFixed(1)}` : "-");
    const draw = Math.max(...run.phases.map((p) => p.drawP95Ms));
    rows.push(`| ${name} | ${run.viewport.name} | ${run.grid} | ${run.tilePxAtFit.toFixed(0)} | ${fmt(ph.pan)} | ${fmt(ph.zoom)} | ${fmt(ph["pan+zoom"])} | ${draw.toFixed(1)} |`);
  }
  for (const h of r.hit) hit.push(`| ${name} | ${h.label} | ${h.clicks} | ${h.misses} |`);
  const g = Object.values(r.gestures ?? {});
  gest.push(`| ${name} | ${g.filter(Boolean).length} of ${g.length} |`);
  misc.push(`| ${name} | ${r.browser} | ${r.os} | ${r.cpus} | ${r.webgl.ok ? r.webgl.renderer : "no WebGL"} | ${r.motion.idleFramesIn1500ms} | ${r.motion.stageBefore} -> ${r.motion.stageAfter} | ${r.motion.reducedMotionAnimates} |`);
}
const out = [
  "| run | window | grid | tile at fit, px | pan fps / p95 ms | zoom fps / p95 ms | pan+zoom fps / p95 ms | draw p95, ms |",
  "|---|---|---|---|---|---|---|---|",
  ...rows,
  "",
  "| run | click-through case | clicks | misses |",
  "|---|---|---|---|",
  ...hit,
  "",
  "| run | gesture and key checks passed |",
  "|---|---|",
  ...gest,
  "",
  "| run | browser | OS | CPU | WebGL renderer | idle frames in 1.5 s | today's stage | animates under reduced motion |",
  "|---|---|---|---|---|---|---|---|",
  ...misc,
  "",
].join("\n");
await writeFile(join(dir, "summary.md"), out);
process.stdout.write(out);
