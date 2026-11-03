// Removes baselines of story visual tests whose stories are no longer in the built Storybook.
// Run from web/ after `npm run build-storybook` and `playwright test --update-snapshots=changed` (see visual-update.yml).
// File names are as in web/tests/visual/stories.spec.ts: `<id>-chromium-linux.png` and `<id>-large-chromium-linux.png`.
// The --dry-run flag only prints the extra files.
import { existsSync, readFileSync, readdirSync, unlinkSync } from "node:fs";

const INDEX = "storybook-static/index.json";
const DIR = "tests/visual/stories.spec.ts-snapshots";
const SUFFIX = "-chromium-linux.png";

const dryRun = process.argv.includes("--dry-run");

if (!existsSync(INDEX) || !existsSync(DIR)) {
  console.error(`${INDEX} and ${DIR} are required: run from web/ after build-storybook`);
  process.exit(1);
}

const index = JSON.parse(readFileSync(INDEX, "utf8"));
const expected = new Set();
for (const entry of Object.values(index.entries)) {
  if (entry.type !== "story") continue;
  expected.add(`${entry.id}${SUFFIX}`);
  expected.add(`${entry.id}-large${SUFFIX}`);
}
if (expected.size === 0) {
  console.error("index.json has no stories: removing nothing");
  process.exit(1);
}

let removed = 0;
for (const file of readdirSync(DIR)) {
  if (!file.endsWith(".png") || expected.has(file)) continue;
  console.log(`${dryRun ? "extra" : "removing"}: ${file}`);
  if (!dryRun) unlinkSync(`${DIR}/${file}`);
  removed += 1;
}
console.log(`baselines without a story: ${removed}`);
