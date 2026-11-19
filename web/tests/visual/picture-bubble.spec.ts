import { expect, test, type Page } from "@playwright/test";

// The frameless bubble of a picture (KINIG-132), measured in a real browser. Like scrollbars.spec.ts these checks measure and do not compare
// with a baseline (the look is in the baselines of the stories "Картинка без рамки").
//   1. No row of the bubble's fill at any edge of a picture that fills it: a phone screenshot (1170 x 2532) has a thumbnail of 296 x 640, a shape
//      that differs from the original's by rounding, and a picture that does not fill its box exactly leaves a row of the fill at its edge.
//   2. The pill, and the time in it, let a click through to the picture (the viewer opens on it): `elementFromPoint` is the picture.

const story = (name: string) => `компоненты-вложения-картинка-без-рамки--${name}`;
const EDGE_BAND = 3; // device pixels
const SAME_COLOR = 3; // per channel: the fill and "the pixel is the fill"
const OFFSETS = [0, 0.25, 0.5, 0.75]; // CSS pixels, both ways
const SCREENSHOT_AT = 1; // in the story "phone-screenshot" the screenshot is the second bubble, below an ordinary photo (as in a feed)

async function open(page: Page, name: string) {
  await page.goto(`/iframe.html?id=${encodeURIComponent(story(name))}&viewMode=story`);
  await page.locator("#storybook-root .message--bleed").first().waitFor();
  await page.evaluate(() => document.fonts.ready);
  // The thumbnail is loaded lazily: wait until it is decoded, so that the picture is what is sampled.
  await page.evaluate(() => Promise.all([...document.images].map((image) => image.decode().catch(() => undefined))));
}

/**
 * How many pixels of the bubble's outermost bands (not its rounded corners) have the colour of the bubble's fill; per edge. The bubble is moved
 * by (dx, dy) CSS pixels first: the browser snaps a picture to whole device pixels, so a row of fill at its edge shows only at some positions
 * (in the feed the bubble's position depends on what is above it), and the check has to look at several.
 */
async function fillAtEdges(page: Page, index: number, dx = 0, dy = 0) {
  await page.evaluate(
    ({ index, dx, dy }) => {
      const element = document.querySelectorAll<HTMLElement>("#storybook-root .message--bleed")[index];
      if (!element) throw new Error("no bubble");
      element.style.marginLeft = `${dx}px`;
      element.style.marginTop = `${dy}px`;
    },
    { index, dx, dy },
  );
  const bubble = page.locator("#storybook-root .message--bleed").nth(index);
  const picture = (await bubble.screenshot()).toString("base64");
  return page.evaluate(
    async ({ picture, band, tolerance, index }) => {
      const bubble = document.querySelectorAll<HTMLElement>("#storybook-root .message--bleed")[index];
      if (!bubble) throw new Error("no bubble");
      const fill = getComputedStyle(document.documentElement).getPropertyValue("--color-surface-message").trim();
      const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(fill);
      if (!match) throw new Error(`the fill is not a hex colour: ${fill}`);
      const want = [match[1], match[2], match[3]].map((hex) => parseInt(hex ?? "0", 16));
      const bytes = Uint8Array.from(atob(picture), (symbol) => symbol.charCodeAt(0));
      const image = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
      const canvas = document.createElement("canvas");
      canvas.width = image.width;
      canvas.height = image.height;
      const context = canvas.getContext("2d");
      if (!context) throw new Error("no canvas");
      context.drawImage(image, 0, 0);
      const { data, width, height } = context.getImageData(0, 0, image.width, image.height);
      const scale = window.devicePixelRatio;
      const corner = Math.ceil(parseFloat(getComputedStyle(bubble).borderTopLeftRadius) * scale) + 1;
      const isFill = (x: number, y: number) => {
        const at = (y * width + x) * 4;
        return [0, 1, 2].every((channel) => Math.abs((data[at + channel] ?? 0) - (want[channel] ?? 0)) <= tolerance);
      };
      const count = { top: 0, bottom: 0, left: 0, right: 0 };
      for (let along = corner; along < width - corner; along++) {
        for (let depth = 0; depth < band; depth++) {
          if (isFill(along, depth)) count.top++;
          if (isFill(along, height - 1 - depth)) count.bottom++;
        }
      }
      for (let along = corner; along < height - corner; along++) {
        for (let depth = 0; depth < band; depth++) {
          if (isFill(depth, along)) count.left++;
          if (isFill(width - 1 - depth, along)) count.right++;
        }
      }
      return { ...count, scale, size: `${width}x${height}`, fill };
    },
    { picture, band: EDGE_BAND, tolerance: SAME_COLOR, index },
  );
}

// KINIG-136, the cover of a hidden photo: it fills the frameless bubble like the picture, the pill stays on it and takes no click, and pressing it
// changes no size (a hidden photo with a description and one without). Measured, not compared with a baseline.
const spoiler = (name: string) => `компоненты-вложения-спойлер--${name}`;

async function openSpoiler(page: Page, name: string, selector: string) {
  await page.goto(`/iframe.html?id=${encodeURIComponent(spoiler(name))}&viewMode=story`);
  await page.locator(selector).first().waitFor();
  await page.evaluate(() => document.fonts.ready);
}

for (const scale of [1, 2]) {
  test.describe(`обложка скрытого фото, ${scale}x`, () => {
    test.use({ deviceScaleFactor: scale });

    test("обложка заполняет безрамочный пузырь, пилюля лежит на ней и пропускает нажатие", async ({ page }) => {
      await openSpoiler(page, "covered-picture-with-pill", "#storybook-root .message--bleed [data-spoiler-face]");
      const boxes = await page.evaluate(() => {
        const rect = (selector: string) => {
          const box = document.querySelector(selector)?.getBoundingClientRect();
          return box ? { left: box.left, top: box.top, width: box.width, height: box.height } : null;
        };
        const pill = document.querySelector<HTMLElement>("#storybook-root .message--bleed .message__meta")?.getBoundingClientRect();
        const hit = pill ? document.elementFromPoint(pill.left + pill.width / 2, pill.top + pill.height / 2) : null;
        return { bubble: rect(".message--bleed"), face: rect("[data-spoiler-face]"), pill: pill ? { width: pill.width } : null, clickable: hit?.closest(".album__open") !== null && hit !== null };
      });
      const { bubble, face } = boxes;
      expect(bubble && face, JSON.stringify(boxes)).toBeTruthy();
      // The cover is the photo's box: as wide and as high as the bubble (a hairline at most, from rounding), at its top left.
      expect(Math.abs((face?.width ?? 0) - (bubble?.width ?? 1)), JSON.stringify(boxes)).toBeLessThanOrEqual(1);
      expect(Math.abs((face?.height ?? 0) - (bubble?.height ?? 1)), JSON.stringify(boxes)).toBeLessThanOrEqual(1);
      expect(Math.abs((face?.left ?? 0) - (bubble?.left ?? 1)) + Math.abs((face?.top ?? 0) - (bubble?.top ?? 1)), JSON.stringify(boxes)).toBeLessThanOrEqual(1);
      expect(boxes.pill?.width ?? 0, JSON.stringify(boxes)).toBeGreaterThan(0);
      expect(boxes.clickable, "a click on the pill reaches the cover").toBe(true);
    });

    test("у краёв обложки нет полосы заливки пузыря (вместе с контролем: заливку видно у узкой полосы)", async ({ page }) => {
      await openSpoiler(page, "covered-picture-with-pill", "#storybook-root .message--bleed [data-spoiler-face]");
      const found: unknown[] = [];
      for (const dy of OFFSETS) {
        for (const dx of OFFSETS) {
          const edges = await fillAtEdges(page, 0, dx, dy);
          if (edges.top + edges.bottom + edges.left + edges.right > 0) found.push({ dx, dy, ...edges });
        }
      }
      expect(found, JSON.stringify(found)).toEqual([]);
      // The control: the tall strip, covered, is narrower than its bubble, and the sampler finds the fill beside it.
      await openSpoiler(page, "covered-tall-strip", "#storybook-root .message--bleed [data-spoiler-face]");
      const strip = await fillAtEdges(page, 0);
      expect(strip.left, JSON.stringify(strip)).toBeGreaterThan(0);
      expect(strip.right, JSON.stringify(strip)).toBeGreaterThan(0);
    });

    test("нажатие на обложку показывает фото и не меняет размеров пузыря (с описанием и без него)", async ({ page }) => {
      await openSpoiler(page, "reveal-in-the-feed", "#storybook-root .message--bleed [data-spoiler-face]");
      const measure = () =>
        page.evaluate(() =>
          [...document.querySelectorAll<HTMLElement>("#storybook-root .feed__item")].map((item) => {
            const box = item.getBoundingClientRect();
            const bubble = item.querySelector<HTMLElement>(".message")?.getBoundingClientRect();
            return { item: [box.width, box.height], bubble: bubble ? [bubble.width, bubble.height] : null };
          }),
        );
      const before = await measure();
      expect(before).toHaveLength(2);
      for (const index of [0, 1]) {
        await page.locator("#storybook-root [data-spoiler-face]").first().click();
        // The reveal swapped the cover for the picture.
        await expect(page.locator("#storybook-root .feed__item").nth(index).locator("img.album__image")).toHaveCount(1);
        await page.evaluate(() => Promise.all([...document.images].map((image) => image.decode().catch(() => undefined))));
      }
      expect(await measure()).toEqual(before);
    });
  });
}

for (const scale of [1, 2]) {
  test.describe(`картинка без рамки, ${scale}x`, () => {
    test.use({ deviceScaleFactor: scale });

    test("скриншот телефона: у краёв картинки нет полосы заливки пузыря", async ({ page }) => {
      await open(page, "phone-screenshot");
      const found: unknown[] = [];
      for (const dy of OFFSETS) {
        for (const dx of OFFSETS) {
          const edges = await fillAtEdges(page, SCREENSHOT_AT, dx, dy);
          expect(edges.scale, JSON.stringify(edges)).toBe(scale);
          if (edges.top + edges.bottom + edges.left + edges.right > 0) found.push({ dx, dy, ...edges });
        }
      }
      expect(found, JSON.stringify(found)).toEqual([]);
    });

    test("контроль: то же измерение видит заливку по бокам узкой картинки", async ({ page }) => {
      await open(page, "narrow-strip");
      const edges = await fillAtEdges(page, 0);
      // The picture is 39 px wide in a bubble of at least the floor width: the strips beside it are the fill, so the sampler has to find them.
      expect(edges.left, JSON.stringify(edges)).toBeGreaterThan(0);
      expect(edges.right, JSON.stringify(edges)).toBeGreaterThan(0);
    });

    for (const target of [".message__meta", ".message__time"]) {
      test(`пилюля не ловит нажатие: центр ${target} это картинка`, async ({ page }) => {
        await open(page, "phone-screenshot");
        const hit = await page.evaluate((selector) => {
          const element = document.querySelector<HTMLElement>(`#storybook-root .message--bleed ${selector}`);
          if (!element) return { error: `no ${selector}` };
          const box = element.getBoundingClientRect();
          const found = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
          return { width: box.width, found: found ? `${found.tagName.toLowerCase()}.${found.className}` : null };
        }, target);
        expect(hit, JSON.stringify(hit)).toEqual({ width: expect.any(Number), found: "img.album__image" });
      });
    }
  });
}
