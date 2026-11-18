import { expect, test, type Page } from "@playwright/test";

// The pending attachment tiles inside the composer's box (KINIG-135), measured in a real browser at the default window size of the app
// (1280 x 800, playwright.config.ts). Like scrollbars.spec.ts these checks measure geometry and do not compare with a baseline.
// Scroll bars are always visible, as with the macOS "Always show scroll bars" setting (the default flag hides them).
test.use({ launchOptions: { ignoreDefaultArgs: ["--hide-scrollbars"] } });

const story = (name: string) => `компоненты-вложения-плитки-над-полем--${name}`;

async function open(page: Page, name: string) {
  await page.goto(`/iframe.html?id=${encodeURIComponent(story(name))}&viewMode=story`);
  await page.locator("#storybook-root > *").first().waitFor();
  await page.evaluate(() => document.fonts.ready);
}

for (const scale of ["default", "large"]) {
  test(`две полные строки плиток помещаются без прокрутки при 1280×800 (${scale})`, async ({ page }) => {
    await open(page, "двестроки");
    if (scale === "large") await page.evaluate(() => document.documentElement.setAttribute("data-text-scale", "large"));
    const area = await page.evaluate(() => {
      const element = document.querySelector<HTMLElement>(".composer__tiles");
      if (!element) throw new Error("no tile area");
      const tops = new Set([...element.querySelectorAll<HTMLElement>(".pending-tile")].map((tile) => Math.round(tile.getBoundingClientRect().top)));
      const style = getComputedStyle(element);
      return {
        lines: tops.size,
        taller: element.scrollHeight - element.clientHeight,
        wider: element.scrollWidth - element.clientWidth,
        verticalBar: element.offsetWidth - element.clientWidth,
        // The cap that bound: it must not cut two lines (max-height in px as computed).
        maxHeight: parseFloat(style.maxHeight),
        height: element.getBoundingClientRect().height,
      };
    });
    // Exactly two lines (otherwise the check proves nothing), all of it visible: no vertical or horizontal overflow, no scroll bar.
    expect(area.lines, JSON.stringify(area)).toBe(2);
    expect(area.taller, JSON.stringify(area)).toBeLessThanOrEqual(0);
    expect(area.wider, JSON.stringify(area)).toBeLessThanOrEqual(0);
    expect(area.verticalBar, JSON.stringify(area)).toBe(0);
  });
}

// The keyboard focus ring (2 px, 2 px away, drawn outside the element) must not be cut by any ancestor that clips: the picture button and the x.
const STORY_WITH_BUTTONS = "двафотоиpdf";
// The picture of the focused composer is a baseline too (`focus-photo.png`, `focus-remove.png`): the keyboard focus is not drivable in the real
// window (the screens scenarios use accessibility actions only), so this is where the ring on a tile is looked at.
for (const [target, selector, shot] of [
  ["кнопка фото", ".pending-tile__open", "focus-photo.png"],
  ["крестик", ".pending-tile__remove", "focus-remove.png"],
] as const) {
  test(`кольцо фокуса не обрезано: ${target}`, async ({ page }) => {
    await open(page, STORY_WITH_BUTTONS);
    // The keyboard, as a person does: Tab until the target has the focus.
    let focused = false;
    for (let step = 0; step < 12 && !focused; step++) {
      await page.keyboard.press("Tab");
      focused = await page.evaluate((query) => document.activeElement?.matches(query) ?? false, selector);
    }
    expect(focused, `Tab never reached ${selector}`).toBe(true);
    const ring = await page.evaluate(() => {
      const element = document.activeElement as HTMLElement;
      const style = getComputedStyle(element);
      const reach = parseFloat(style.outlineWidth) + parseFloat(style.outlineOffset);
      const box = element.getBoundingClientRect();
      const cuts: { by: string; px: number }[] = [];
      for (let parent = element.parentElement; parent && parent !== document.body && parent !== document.documentElement; parent = parent.parentElement) {
        const clip = getComputedStyle(parent);
        if (clip.overflowX === "visible" && clip.overflowY === "visible") continue;
        const edge = parent.getBoundingClientRect();
        const inner = { left: edge.left + parent.clientLeft, top: edge.top + parent.clientTop };
        const px = Math.max(
          inner.left - (box.left - reach),
          inner.top - (box.top - reach),
          box.right + reach - (inner.left + parent.clientWidth),
          box.bottom + reach - (inner.top + parent.clientHeight),
        );
        if (px > 0.5) cuts.push({ by: `${parent.tagName.toLowerCase()}.${parent.className}`, px });
      }
      return { drawn: style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0, cuts };
    });
    expect(ring.drawn, "the focus ring is not drawn").toBe(true);
    expect(ring.cuts, JSON.stringify(ring.cuts)).toEqual([]);
    await expect(page.locator(".composer")).toHaveScreenshot(shot);
  });
}
