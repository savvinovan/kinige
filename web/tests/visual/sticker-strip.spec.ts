import { expect, test, type Page } from "@playwright/test";

// The pack strip of the Stickers tab (slice 5c), measured in a real browser: jsdom has no layout, so the mark that follows the grid is checked here. Like
// reactions.spec.ts this compares no baseline.

const STORY = "компоненты-стикеры-панель--всевкладки";

async function open(page: Page) {
  await page.goto(`/iframe.html?id=${encodeURIComponent(STORY)}&viewMode=story`);
  await page.locator(".sticker-strip__item").first().waitFor();
  await page.evaluate(() => document.fonts.ready);
}

test("полоска наборов отмечает последний набор в конце сетки, даже короткий", async ({ page }) => {
  await open(page);
  const icons = page.locator(".sticker-strip__item");
  const scroll = page.locator(".sticker-panel__scroll");
  const marked = (index: number) => icons.nth(index).getAttribute("aria-current");

  // The case under test: the last section is shorter than the grid, so its top can never reach the top of the grid.
  const heights = await page.evaluate(() => ({
    area: (document.querySelector(".sticker-panel__scroll") as HTMLElement).clientHeight,
    last: [...document.querySelectorAll<HTMLElement>(".sticker-panel__section")].at(-1)?.offsetHeight ?? 0,
  }));
  expect(heights.last, JSON.stringify(heights)).toBeLessThan(heights.area);

  await expect(icons.nth(0)).toHaveAttribute("aria-current", "true");
  await scroll.evaluate((node) => node.scrollTo({ top: node.scrollHeight }));
  await expect(icons.nth(1)).toHaveAttribute("aria-current", "true");
  expect(await marked(0)).toBeNull();

  // A click on an icon moves the grid, and the mark stays on that icon, the last one too (it did go back to the pack before it).
  await icons.nth(0).click();
  await expect(icons.nth(0)).toHaveAttribute("aria-current", "true");
  await icons.nth(1).click();
  await expect(icons.nth(1)).toHaveAttribute("aria-current", "true");
  expect(await marked(0)).toBeNull();
  // The "+" is never marked.
  expect(await marked(2)).toBeNull();
});
