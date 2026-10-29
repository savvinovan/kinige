import { expect, test, type Page } from "@playwright/test";

// Полосы прокрутки видны всегда, как при настройке macOS «Всегда показывать полосы прокрутки» с мышью (KINIG-40, KINIG-38).
// По умолчанию Playwright запускает Chromium с --hide-scrollbars, и снимки историй полос не видят вовсе: здесь флаг снят.
// Проверки считают геометрию и цвет пикселей, а не сравнивают с эталоном, поэтому отдельных эталонов не нужно.
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
      if (!pre || !next) throw new Error("в истории нет блока кода и абзаца после него");
      return {
        // offsetHeight включает полосу прокрутки, clientHeight нет: разница — высота полосы под блоком.
        scrollbar: pre.offsetHeight - pre.clientHeight,
        horizontalOverflow: pre.scrollWidth - pre.clientWidth,
        gap: next.getBoundingClientRect().top - pre.getBoundingClientRect().bottom,
        lines: Math.round(pre.clientHeight / parseFloat(getComputedStyle(pre).lineHeight)),
      };
    });

    expect(geometry.scrollbar).toBe(0);
    expect(geometry.horizontalOverflow).toBeLessThanOrEqual(0);
    expect(geometry.gap).toBeGreaterThan(0);
    // Длинная строка действительно перенеслась (иначе тест ничего не проверял бы).
    expect(geometry.lines).toBeGreaterThan(2);
  });
}

test("полосы прокрутки тёмные, а не светлые на тёмном фоне", async ({ page }) => {
  await open(page, STORY);
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).colorScheme)).toBe("dark");

  // Прокручиваемая область поверх истории: полоса справа, её дорожка — системная.
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
  // Пиксели читаются тем же браузером: отдельного разборщика PNG в зависимостях нет.
  const mean = await page.evaluate(async (base64) => {
    const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${base64}`)).blob());
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext("2d");
    if (!context) throw new Error("нет 2d-контекста");
    context.drawImage(bitmap, 0, 0);
    const { data } = context.getImageData(Math.floor(bitmap.width / 2), 5, 1, 1);
    return (0.2126 * (data[0] ?? 0) + 0.7152 * (data[1] ?? 0) + 0.0722 * (data[2] ?? 0)) / 255;
  }, shot.toString("base64"));
  expect(mean).toBeLessThan(0.35);
});
