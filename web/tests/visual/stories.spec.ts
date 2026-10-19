import { expect, test } from "@playwright/test";

interface StorybookIndex {
  entries: Record<string, { id: string; type: string }>;
}

// Одна тема, обычный размер текста (крупный текст — со слайса D1).
test("снимки всех историй", async ({ page, request }) => {
  const index = (await (await request.get("/index.json")).json()) as StorybookIndex;
  const stories = Object.values(index.entries).filter((entry) => entry.type === "story");
  expect(stories.length).toBeGreaterThan(0);

  for (const story of stories) {
    await page.goto(`/iframe.html?id=${encodeURIComponent(story.id)}&viewMode=story`);
    await page.locator("#storybook-root > *").first().waitFor();
    await page.evaluate(() => document.fonts.ready);
    await expect(page).toHaveScreenshot(`${story.id}.png`);
  }
});
