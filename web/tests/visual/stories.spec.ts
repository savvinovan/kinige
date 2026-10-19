import { readFileSync, readdirSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";

interface StorybookIndex {
  entries: Record<string, { id: string; type: string }>;
}

// Одна тема. Каждая история снимается дважды: обычный и крупный текст (атрибут data-text-scale на <html>, как в приложении).
// Список историй берётся из собранного Storybook (`npm run build-storybook` идёт до тестов).
const index = JSON.parse(readFileSync("storybook-static/index.json", "utf8")) as StorybookIndex;
const stories = Object.values(index.entries).filter((entry) => entry.type === "story");

async function open(page: Page, id: string) {
  await page.goto(`/iframe.html?id=${encodeURIComponent(id)}&viewMode=story`);
  await page.locator("#storybook-root > *").first().waitFor();
  await settle(page);
}

/** Шрифты грузятся лениво, когда текст уже разложен: ждём кадр, потом готовность. */
async function settle(page: Page) {
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise((resolve) => requestAnimationFrame(resolve));
    await document.fonts.ready;
  });
}

test("есть истории для снимков", () => {
  expect(stories.length).toBeGreaterThan(0);
});

for (const story of stories) {
  test(`снимок: ${story.id}`, async ({ page }) => {
    await open(page, story.id);
    await expect(page).toHaveScreenshot(`${story.id}.png`, { fullPage: true });

    await page.evaluate(() => document.documentElement.setAttribute("data-text-scale", "large"));
    await settle(page);
    await expect(page).toHaveScreenshot(`${story.id}-large.png`, { fullPage: true });
  });
}

// Файлы из токенов действительно загрузились и декодировались браузером (а не подменились системным шрифтом).
test("встроенные шрифты загружаются в браузере", async ({ page }) => {
  const dir = "../design/tokens/fontsets";
  const faces = readdirSync(dir).flatMap((file) => {
    const set = (JSON.parse(readFileSync(`${dir}/${file}`, "utf8")) as { fontset: Record<string, { face: Record<string, { $value: Record<string, string> }> }> }).fontset;
    return Object.values(set).flatMap((entry) => Object.values(entry.face).map((face) => face.$value));
  });
  expect(faces.length).toBeGreaterThan(0);

  await open(page, "основа-типографика--кандидаты");
  const loaded = await page.evaluate(() =>
    [...document.fonts]
      .filter((font) => font.status === "loaded")
      .map((font) => `${font.family.replaceAll('"', "")}|${font.style}|${font.weight}`),
  );
  for (const face of faces) {
    expect(loaded, `${face.family} ${face.style} ${face.weight}`).toContain(`${face.family}|${face.style}|${face.weight}`);
  }
});
