import { expect, test, type Page } from "@playwright/test";

// Scroll bars are always visible, as with the macOS "Always show scroll bars" setting with a mouse (KINIG-40, KINIG-38).
// By default Playwright launches Chromium with --hide-scrollbars, and story snapshots do not see scroll bars at all: here the flag is removed.
// The checks measure geometry and pixel color rather than compare with a baseline, so no separate baselines are needed.
test.use({ launchOptions: { ignoreDefaultArgs: ["--hide-scrollbars"] } });

const STORY = "компоненты-сообщение--кодсдлиннойстрокой";

async function open(page: Page, id: string) {
  await page.goto(`/iframe.html?id=${encodeURIComponent(id)}&viewMode=story`);
  await page.locator("#storybook-root > *").first().waitFor();
  await page.evaluate(() => document.fonts.ready);
}

for (const scale of ["default", "large"]) {
  test(`блок кода с длинной строкой: полосы нет, абзац ниже не закрыт (${scale})`, async ({ page }) => {
    await open(page, STORY);
    if (scale === "large") await page.evaluate(() => document.documentElement.setAttribute("data-text-scale", "large"));

    const geometry = await page.evaluate(() => {
      const pre = document.querySelector<HTMLElement>(".markdown pre");
      const next = pre?.nextElementSibling;
      if (!pre || !next) throw new Error("the story has no code block and no paragraph after it");
      return {
        // offsetHeight includes the scroll bar, clientHeight does not: the difference is the bar height under the block.
        scrollbar: pre.offsetHeight - pre.clientHeight,
        horizontalOverflow: pre.scrollWidth - pre.clientWidth,
        gap: next.getBoundingClientRect().top - pre.getBoundingClientRect().bottom,
        lines: Math.round(pre.clientHeight / parseFloat(getComputedStyle(pre).lineHeight)),
      };
    });

    expect(geometry.scrollbar).toBe(0);
    expect(geometry.horizontalOverflow).toBeLessThanOrEqual(0);
    expect(geometry.gap).toBeGreaterThan(0);
    // The long line really wrapped (otherwise the test would check nothing).
    expect(geometry.lines).toBeGreaterThan(2);
  });
}

test("полосы прокрутки тёмные, а не светлые на тёмном фоне", async ({ page }) => {
  await open(page, STORY);
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).colorScheme)).toBe("dark");

  // A scrollable area over the story: the bar is on the right, its track is the system one.
  const track = await page.evaluate(() => {
    const box = document.createElement("div");
    box.id = "scrollbar-probe";
    box.style.cssText = "position:fixed;left:0;top:0;width:200px;height:100px;overflow:scroll;";
    box.innerHTML = '<div style="width:600px;height:600px"></div>';
    document.body.append(box);
    return { width: box.offsetWidth - box.clientWidth };
  });
  expect(track.width).toBeGreaterThan(0);
  const shot = await page.screenshot({ clip: { x: 200 - track.width, y: 40, width: track.width, height: 20 } });
  // The pixels are read by the same browser: there is no separate PNG parser in the dependencies.
  const mean = await page.evaluate(async (base64) => {
    const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${base64}`)).blob());
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext("2d");
    if (!context) throw new Error("no 2d context");
    context.drawImage(bitmap, 0, 0);
    const { data } = context.getImageData(Math.floor(bitmap.width / 2), 5, 1, 1);
    return (0.2126 * (data[0] ?? 0) + 0.7152 * (data[1] ?? 0) + 0.0722 * (data[2] ?? 0)) / 255;
  }, shot.toString("base64"));
  expect(mean).toBeLessThan(0.35);
});

// The "Настройки" (Settings) window (slice 4b): the only scrollable area is the page on the right (`.prefs__page`). The bars are always visible
// (the `--hide-scrollbars` flag is removed above), as with "Always show scroll bars". The tree on the left, the chat lists, the log,
// the code blocks and the paths have no scrolling of their own; the provider warning lies over the window and does not move the page under it.
const PREFS = (story: string) => `экраны-окно-настроек--${story}`;
const PREFS_STORIES = ["оформление", "доступвыключен", "видимыечаты", "журналдлинный", "файлбезошибок", "ошибкиключей", "длинныепутииназвания", "наименьшийразмер", "наименьшийразмерподэкран"];

for (const story of PREFS_STORIES) {
  for (const scale of ["default", "large"]) {
    test(`окно настроек «${story}» (${scale}): единственный прокручиваемый блок — страница справа`, async ({ page }) => {
      await open(page, PREFS(story));
      if (scale === "large") await page.evaluate(() => document.documentElement.setAttribute("data-text-scale", "large"));
      const scrollers = await page.evaluate(() =>
        [...document.querySelectorAll<HTMLElement>("#storybook-root *")]
          .filter((element) => getComputedStyle(element).display !== "inline")
          .map((element) => {
            const style = getComputedStyle(element);
            const border = (side: string) => parseFloat(style.getPropertyValue(`border-${side}-width`)) || 0;
            return {
              name: `${element.tagName.toLowerCase()}.${element.className}`,
              overflow: `${style.overflowX}/${style.overflowY}`,
              verticalBar: element.offsetWidth - element.clientWidth - border("left") - border("right"),
              horizontalBar: element.offsetHeight - element.clientHeight - border("top") - border("bottom"),
              taller: element.scrollHeight - element.clientHeight,
              hidden: style.overflowY !== "visible",
            };
          })
          // The story frame clips the canvas itself (`overflow: clip`): this is not scrolling.
          .filter((item) => !item.name.includes("story-prefs-frame"))
          .filter((item) => item.verticalBar > 0 || item.horizontalBar > 0 || (item.hidden && item.taller > 0 && !item.overflow.includes("clip"))),
      );
      const names = scrollers.map((item) => item.name);
      expect(names.every((name) => name.includes("prefs__page")), JSON.stringify(scrollers)).toBe(true);
    });
  }
}

test("окно настроек: дерево при наименьшем окне и крупном тексте помещается без прокрутки", async ({ page }) => {
  await open(page, PREFS("наименьшийразмер"));
  await page.evaluate(() => document.documentElement.setAttribute("data-text-scale", "large"));
  const tree = await page.evaluate(() => {
    const nav = document.querySelector<HTMLElement>(".settings-tree");
    if (!nav) throw new Error("no tree");
    return { scrollable: nav.scrollHeight - nav.clientHeight, bottom: nav.getBoundingClientRect().bottom, lastBottom: [...nav.querySelectorAll("button")].at(-1)!.getBoundingClientRect().bottom };
  });
  expect(tree.scrollable).toBeLessThanOrEqual(0);
  expect(tree.lastBottom).toBeLessThanOrEqual(tree.bottom);
});

test("окно настроек: предупреждение не сдвигает страницу под ним и не меняет её ширину, даже с постоянными полосами", async ({ page }) => {
  const measure = () =>
    page.evaluate(() => {
      const scroller = document.querySelector<HTMLElement>(".prefs__page");
      const heading = document.querySelector<HTMLElement>(".prefs-page__title");
      if (!scroller || !heading) throw new Error("no page");
      const rect = heading.getBoundingClientRect();
      const box = scroller.getBoundingClientRect();
      return {
        clientWidth: scroller.clientWidth,
        offsetWidth: scroller.offsetWidth,
        overflowY: getComputedStyle(scroller).overflowY,
        headingLeft: rect.left,
        headingTop: rect.top,
        pageLeft: box.left,
        pageWidth: box.width,
      };
    });
  await open(page, PREFS("доступвыключен"));
  const without = await measure();
  await open(page, PREFS("предупреждениепривключении"));
  await expect(page.locator(".modal__panel")).toBeVisible();
  const withWarning = await measure();
  expect(withWarning).toEqual(without);
});

test("окно настроек: переход по дереву и «назад» открывают страницу сверху, а не с места прошлой прокрутки", async ({ page }) => {
  await open(page, PREFS("доступвыключен"));
  await page.evaluate(() => document.documentElement.setAttribute("data-text-scale", "large"));
  const top = () => page.evaluate(() => document.querySelector<HTMLElement>(".prefs__page")!.scrollTop);
  const scrollBy300 = () => page.evaluate(() => (document.querySelector<HTMLElement>(".prefs__page")!.scrollTop = 300));
  await scrollBy300();
  expect(await top()).toBeGreaterThan(0);
  await page.getByRole("button", { name: "Копии" }).click();
  expect(await top()).toBe(0);
  await page.getByRole("button", { name: "Доступ ИИ-агентов" }).click();
  await scrollBy300();
  await page.getByRole("button", { name: /Журнал обращений.*Настроить/ }).click();
  expect(await top()).toBe(0);
});
