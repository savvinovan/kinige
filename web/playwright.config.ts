import { defineConfig, devices } from "@playwright/test";

// Снимки историй Storybook. Эталоны снимаются на GitHub (.github/workflows/visual-update.yml)
// тем же образом, что и в CI, поэтому `*-linux.png` — единственные эталоны в репозитории.
export default defineConfig({
  testDir: "tests/visual",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  // One retry in CI for one-off load hiccups; it does not cure the 2-pixel difference (see maxDiffPixels below).
  retries: process.env.CI ? 1 : 0,
  reporter: [["list"]],
  // reducedMotion: переходы дизайн-системы при Reduce Motion нулевые (themes.css), и снимок не может поймать их на середине.
  // Без этого поле ввода в фокусе (смена заливки + линия фокуса у скруглённого угла) в ~1% прогонов давало расхождение в 2 пикселя
  // на ±1 уровень: `animations: "disabled"` не всегда успевает довести CSS-переход до конца.
  use: { baseURL: "http://127.0.0.1:6007", reducedMotion: "reduce" },
  // threshold: 0 — по умолчанию 0.2, и тогда заметная смена фона проходит как «то же самое».
  // maxDiffPixels: 8 — rasterization noise, not page state (KINIG-48). The artifacts of failed runs (31.10-01.11.2026) show
  // 2 pixels off by 1 level of a channel, exactly at the rounded corners of a focused field with an error (the error border
  // and the inner focus line share one radius: two anti-aliased edges on top of each other; "Rename chat" with an error, the
  // send error). The retry in the same job a few seconds later gives the same values, so it is not a transition, a caret or
  // focus: the likely cause is runners with different CPUs rounding Skia's anti-aliasing differently (not verified: the
  // rasterizer was not pinned with Chromium flags). A real regression (a colour, a shifted line, text) touches hundreds of
  // pixels and does not fit in 8.
  expect: { toHaveScreenshot: { animations: "disabled", threshold: 0, maxDiffPixels: 8 } },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } },
    },
  ],
  webServer: {
    command: "npx http-server storybook-static --port 6007 --address 127.0.0.1 --silent",
    url: "http://127.0.0.1:6007/index.json",
    reuseExistingServer: false,
  },
});
