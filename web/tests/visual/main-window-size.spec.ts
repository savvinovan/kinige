import { expect, test, type Page } from "@playwright/test";

// The smallest main window, 640 x 400 (KINIG-76, KINIG-150): the document never scrolls in either direction, the top bar with the forest switch,
// and the field stay inside the window, and the chat name in the bar is one line with the switch at the same place whatever the name is. Geometry is measured,
// not compared with a baseline. Scroll bars are always visible, as with the macOS "Always show scroll bars" setting.
test.use({ launchOptions: { ignoreDefaultArgs: ["--hide-scrollbars"] }, viewport: { width: 640, height: 400 } });

const MIN = (story: string) => `экраны-главное-окно-наименьшее-окно--${story}`;
const FOREST = (story: string) => `экраны-главное-окно-лес--${story}`;
const SLICE5 = (story: string) => `экраны-главное-окно-в-этот-день-и-ответ--${story}`;

// The framed stories are measured inside their frame (it is the window); the others fill the 640 x 400 viewport.
const FRAMED = ["hidden", "hidden-large-text", "strip", "strip-large-text", "expanded", "expanded-large-text", "reply-strip-hidden", "reply-strip-strip-large-text", "reply-strip-expanded-large-text"].map(MIN);
const UNFRAMED = [FOREST("hidden"), FOREST("strip-above-the-feed"), FOREST("expanded-above-the-feed"), FOREST("title-very-long-name"), FOREST("many-chats"), SLICE5("ответназапись"), SLICE5("английскийкрупный")];

async function open(page: Page, id: string, scale: string) {
  await page.goto(`/iframe.html?id=${encodeURIComponent(id)}&viewMode=story`);
  await page.locator("#storybook-root .main-window").waitFor();
  if (scale === "large") await page.evaluate(() => document.documentElement.setAttribute("data-text-scale", "large"));
  await page.evaluate(() => document.fonts.ready);
}

for (const scale of ["default", "large"]) {
  for (const id of UNFRAMED) {
    test(`${id} (${scale}): the page does not scroll, the field and the switch are inside the window`, async ({ page }) => {
      await open(page, id, scale);
      const metrics = await page.evaluate(() => {
        const root = document.documentElement;
        const inside = (selector: string) => {
          const box = document.querySelector<HTMLElement>(selector)?.getBoundingClientRect();
          return box ? box.left >= 0 && box.top >= 0 && box.right <= innerWidth && box.bottom <= innerHeight : null;
        };
        return {
          wider: root.scrollWidth - innerWidth,
          taller: root.scrollHeight - innerHeight,
          composer: inside(".composer"),
          // The field is the top element at its own centre: a feed squeezed to nothing must not paint or catch clicks over it (the real window at 372 px
          // inner height, KINIG-76 screens, had the last entry over the field).
          uncovered: (() => {
            const box = document.querySelector<HTMLElement>(".composer")?.getBoundingClientRect();
            const hit = box ? document.elementFromPoint(box.left + box.width / 2, box.top + 20) : null;
            return Boolean(hit?.closest(".composer"));
          })(),
          toggle: inside(".forest-toggle"),
          title: inside(".main-window__title"),
        };
      });
      expect(metrics.wider).toBeLessThanOrEqual(0);
      expect(metrics.taller).toBeLessThanOrEqual(0);
      expect(metrics.composer).toBe(true);
      expect(metrics.uncovered).toBe(true);
      expect(metrics.toggle).not.toBe(false);
      expect(metrics.title).toBe(true);
    });
  }

  for (const id of FRAMED) {
    test(`${id} (${scale}): nothing in the window is wider or taller than the window, the field is whole at the bottom`, async ({ page }) => {
      await open(page, id, scale);
      const metrics = await page.evaluate(() => {
        const frame = document.querySelector<HTMLElement>(".story-main-min")?.getBoundingClientRect();
        const chat = document.querySelector<HTMLElement>(".main-window__chat");
        const composer = document.querySelector<HTMLElement>(".composer")?.getBoundingClientRect();
        if (!frame || !chat || !composer) throw new Error("the story has no frame, chat or composer");
        return {
          chatWider: chat.scrollWidth - chat.clientWidth,
          chatTaller: chat.scrollHeight - chat.clientHeight,
          composerBottom: composer.bottom - frame.bottom,
          composerRight: composer.right - frame.right,
        };
      });
      expect(metrics.chatWider).toBeLessThanOrEqual(0);
      expect(metrics.chatTaller).toBeLessThanOrEqual(0);
      expect(metrics.composerBottom).toBeLessThanOrEqual(1);
      expect(metrics.composerRight).toBeLessThanOrEqual(0);
    });
  }
}

// A window 28 px lower than the smallest (the runner's 400 px outer window has a 372 px page, the title bar taking the rest): the expanded board still
// leaves the field whole and uncovered, the feed gives way to nothing and shows nothing over it. Fails without the composer's own layer (`composer.css`).
test("the expanded board at 372 px of height at large text: the feed squeezed to nothing paints nothing over the field", async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 372 });
  await open(page, FOREST("expanded-above-the-feed"), "large");
  const hit = await page.evaluate(() => {
    const box = document.querySelector<HTMLElement>(".composer")!.getBoundingClientRect();
    const top = document.elementFromPoint(box.left + box.width / 2, box.top + 20);
    return { composerBottom: box.bottom - innerHeight, covered: !top?.closest(".composer"), by: String(top?.className ?? "") };
  });
  expect(hit.composerBottom).toBeLessThanOrEqual(0);
  expect(hit.covered, hit.by).toBe(false);
});

// The same bar for every title and for both states of the forest: the switch's box does not move (KINIG-150).
test("the forest switch is in the same place and the bar has the same height for Inbox, a 200-character name, shown and hidden", async ({ page }) => {
  const boxes = [] as { bar: number; toggleLeft: number; toggleTop: number; oneLine: boolean }[];
  for (const id of [FOREST("title-short-name"), FOREST("title-very-long-name"), FOREST("title-very-long-name-hidden")]) {
    await page.setViewportSize({ width: 1280, height: 800 });
    await open(page, id, "default");
    boxes.push(
      await page.evaluate(() => {
        const bar = document.querySelector<HTMLElement>(".main-window__topbar")!.getBoundingClientRect();
        const toggle = document.querySelector<HTMLElement>(".forest-toggle")!.getBoundingClientRect();
        const title = document.querySelector<HTMLElement>(".main-window__title")!;
        return { bar: bar.height, toggleLeft: toggle.left, toggleTop: toggle.top, oneLine: title.scrollHeight <= title.clientHeight };
      }),
    );
  }
  expect(boxes.every((box) => box.oneLine)).toBe(true);
  // The two shown ones (short and long name) agree to the pixel; the hidden one is in a 585 px column, so only its height and top are the same.
  expect(boxes[0]).toEqual(boxes[1]);
  expect(boxes[2]?.bar).toBe(boxes[0]?.bar);
  expect(boxes[2]?.toggleTop).toBe(boxes[0]?.toggleTop);
});
