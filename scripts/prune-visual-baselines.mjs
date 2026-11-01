// Удаляет эталоны визуальных тестов историй, которых больше нет в собранном Storybook.
// Запуск из web/ после `npm run build-storybook` и `playwright test --update-snapshots=changed` (см. visual-update.yml).
// Имена файлов — как в web/tests/visual/stories.spec.ts: `<id>-chromium-linux.png` и `<id>-large-chromium-linux.png`.
// Флаг --dry-run только печатает лишние файлы.
import { existsSync, readFileSync, readdirSync, unlinkSync } from "node:fs";

const INDEX = "storybook-static/index.json";
const DIR = "tests/visual/stories.spec.ts-snapshots";
const SUFFIX = "-chromium-linux.png";

const dryRun = process.argv.includes("--dry-run");

if (!existsSync(INDEX) || !existsSync(DIR)) {
  console.error(`нужны ${INDEX} и ${DIR}: запускайте из web/ после build-storybook`);
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
  console.error("в index.json нет историй: ничего не удаляю");
  process.exit(1);
}

let removed = 0;
for (const file of readdirSync(DIR)) {
  if (!file.endsWith(".png") || expected.has(file)) continue;
  console.log(`${dryRun ? "лишний" : "удаляю"}: ${file}`);
  if (!dryRun) unlinkSync(`${DIR}/${file}`);
  removed += 1;
}
console.log(`эталонов без истории: ${removed}`);
