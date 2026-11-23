import { expect, test, type Page } from "@playwright/test";

// Reactions (slice 5a, KINIG-138), measured in a real browser. Like pending-tiles.spec.ts these checks measure geometry and do not compare with a baseline.

const story = (name: string) => `компоненты-реакции--${name}`;

async function open(page: Page, name: string) {
  await page.goto(`/iframe.html?id=${encodeURIComponent(story(name))}&viewMode=story`);
  await page.locator("#storybook-root > *").first().waitFor();
  await page.evaluate(() => document.fonts.ready);
}

// The feed is `column-reverse`: with the scroll offset kept, a bubble that grows keeps its bottom edge and grows upwards. The bar stands level with the
// bubble's bottom line, so a click on an emoji (which adds a row of chips) must not move it: the next click lands where the pointer is.
for (const scale of ["default", "large"]) {
  test(`ряд действий не сдвигается, когда под текстом появляется ряд чипов (${scale})`, async ({ page }) => {
    await open(page, "безскачка");
    if (scale === "large") await page.evaluate(() => document.documentElement.setAttribute("data-text-scale", "large"));
    const box = async () => {
      const bar = await page.locator(".message-actions__bar").boundingBox();
      const bubble = await page.locator("article.message").boundingBox();
      if (!bar || !bubble) throw new Error("no bar or bubble");
      return { bar, bubble };
    };
    const before = await box();
    await page.locator(".reaction-button").first().click();
    await expect(page.locator(".reaction-chip")).toHaveCount(1);
    const after = await box();
    // The bubble grew (a row of chips), upwards; the bar did not move by a pixel.
    expect(after.bubble.height, JSON.stringify({ before, after })).toBeGreaterThan(before.bubble.height + 10);
    expect(Math.abs(after.bar.y - before.bar.y), JSON.stringify({ before, after })).toBeLessThan(1);
    expect(Math.abs(after.bar.x - before.bar.x), JSON.stringify({ before, after })).toBeLessThan(1);
    expect(Math.abs(after.bubble.y + after.bubble.height - (before.bubble.y + before.bubble.height))).toBeLessThan(1);
  });
}

// A full-width bubble (the bar does not fit to its right, so it stands over its top edge) in the real feed's structure: a put and a removal grow the bubble
// upwards, and the pressed button must stay under the pointer (within a pixel), with the feed at its end and scrolled to the middle.
for (const where of ["конце", "середине"]) {
  for (const scale of ["default", "large"]) {
    test(`широкая запись в ${where}: нажатая кнопка ряда остаётся под указателем при добавлении и снятии (${scale})`, async ({ page }) => {
      await page.goto(`/iframe.html?id=${encodeURIComponent(`компоненты-реакции-в-ленте--широкаязаписьв${where}`)}&viewMode=story`);
      await page.locator("li[data-target] article.message").waitFor();
      await page.evaluate(() => document.fonts.ready);
      if (scale === "large") await page.evaluate(() => document.documentElement.setAttribute("data-text-scale", "large"));
      if (where === "середине") await page.locator("li[data-target]").evaluate((node) => node.scrollIntoView({ block: "center" }));
      const bubble = page.locator("li[data-target] article.message");
      await bubble.hover();
      const wrap = page.locator("li[data-target] .message-wrap");
      await expect(wrap).toHaveClass(/message-wrap--actions-inside/); // the case under test: the bar does not fit on the right
      const button = page.locator("li[data-target] .reaction-button").first();
      const center = async () => {
        const box = await button.boundingBox();
        if (!box) throw new Error("no button");
        return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
      };
      // At rest the bar does not cover the text more than its top padding (what the bar on main did).
      const first = await page.locator("li[data-target] .message__content").boundingBox();
      const barBox = await page.locator("li[data-target] .message-actions__bar").boundingBox();
      if (!first || !barBox) throw new Error("no content or bar");
      expect(barBox.y + barBox.height, JSON.stringify({ first, barBox })).toBeLessThanOrEqual(first.y + 1);

      const before = await center();
      await button.click(); // puts the heart: a row of chips, the bubble grows upwards
      await expect(page.locator("li[data-target] .reaction-chip")).toHaveCount(1);
      const afterPut = await center();
      expect(Math.abs(afterPut.y - before.y), JSON.stringify({ before, afterPut })).toBeLessThan(1);
      expect(Math.abs(afterPut.x - before.x)).toBeLessThan(1);
      // The pointer is where it was: the element under it is still that button.
      const under = await page.evaluate(({ x, y }) => document.elementFromPoint(x, y)?.closest(".reaction-button")?.getAttribute("aria-pressed"), before);
      expect(under).toBe("true");
      await button.click(); // removes it: the bubble shrinks
      await expect(page.locator("li[data-target] .reaction-chip")).toHaveCount(0);
      const afterRemove = await center();
      expect(Math.abs(afterRemove.y - before.y), JSON.stringify({ before, afterRemove })).toBeLessThan(1);
    });
  }
}

// The time sits at the right end of the row of chips, level with its last line (not above or below it).
test("время на одной линии с рядом чипов, у правого края плашки", async ({ page }) => {
  await open(page, "чиподин");
  const [time, chip, bubble] = await Promise.all([
    page.locator(".message__time").boundingBox(),
    page.locator(".reaction-chip").first().boundingBox(),
    page.locator("article.message").boundingBox(),
  ]);
  if (!time || !chip || !bubble) throw new Error("no time, chip or bubble");
  expect(Math.abs(time.y + time.height / 2 - (chip.y + chip.height / 2))).toBeLessThan(4);
  expect(bubble.x + bubble.width - (time.x + time.width)).toBeLessThan(20);
  expect(time.x).toBeGreaterThan(chip.x + chip.width);
});

// Ten chips wrap inside the bubble: nothing sticks out of it and the time does not overlap the last chip.
test("десять чипов переносятся внутри плашки, время не перекрывает чипы", async ({ page }) => {
  await open(page, "чипыдесять");
  const bubble = await page.locator("article.message").boundingBox();
  const time = await page.locator(".message__time").boundingBox();
  const chips = await page.locator(".reaction-chip").evaluateAll((nodes) => nodes.map((node) => node.getBoundingClientRect().toJSON()));
  if (!bubble || !time) throw new Error("no bubble or time");
  expect(chips).toHaveLength(10);
  for (const chip of chips as { x: number; y: number; right: number; bottom: number }[]) {
    expect(chip.right).toBeLessThanOrEqual(bubble.x + bubble.width + 0.5);
    const overlaps = chip.right > time.x && chip.x < time.x + time.width && chip.bottom > time.y && chip.y < time.y + time.height;
    expect(overlaps, JSON.stringify({ chip, time })).toBe(false);
  }
});
