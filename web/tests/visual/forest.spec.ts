import { expect, test, type Page } from "@playwright/test";

// The click-through of the forest (slice 6): every day that can be opened is clicked with a real pointer on the expanded
// board, and the board must open exactly that day. 0 misses. Two years: 2026 (Monday first, 53 columns) and 2028 (Sunday
// first, 54 columns). The stories `ClickThrough*` expose where each day is (`window.__forestProbe`).
const STORIES = [
  { id: "forest-pane-and-board--click-through-2026", year: "2026", days: 294 },
  { id: "forest-pane-and-board--click-through-2028-sunday", year: "2028", days: 167 },
];

async function open(page: Page, id: string) {
  await page.goto(`/iframe.html?id=${encodeURIComponent(id)}&viewMode=story`);
  await page.locator('[data-testid="opened"][data-ready="yes"]').waitFor();
  // The first frame is drawn on the next animation frame, after the fonts.
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  });
}

for (const story of STORIES) {
  test(`click-through: every day of ${story.year} opens that day, 0 misses`, async ({ page }) => {
    await open(page, story.id);
    const days = await page.evaluate(() => window.__forestProbe?.() ?? []);
    expect(days).toHaveLength(story.days);
    const misses: string[] = [];
    for (const day of days) {
      // clickCount 1: two quick clicks on neighbouring cells must not become a double click (a zoom).
      await page.mouse.click(day.x, day.y, { clickCount: 1 });
      try {
        await expect(page.locator('[data-testid="opened"]')).toHaveText(day.date, { timeout: 1000 });
      } catch {
        misses.push(`${day.date} opened ${(await page.locator('[data-testid="opened"]').textContent()) || "nothing"}`);
      }
    }
    expect(misses).toEqual([]);
  });
}
