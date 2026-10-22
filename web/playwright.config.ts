import { defineConfig, devices } from "@playwright/test";

// Снимки историй Storybook. Эталоны снимаются в Linux-контейнере (scripts/visual-tests.sh),
// тем же образом, что и в CI, поэтому `*-linux.png` — единственные эталоны в репозитории.
export default defineConfig({
  testDir: "tests/visual",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  reporter: [["list"]],
  // reducedMotion: переходы дизайн-системы при Reduce Motion нулевые (themes.css), и снимок не может поймать их на середине.
  // Без этого поле ввода в фокусе (смена заливки + линия фокуса у скруглённого угла) в ~1% прогонов давало расхождение в 2 пикселя
  // на ±1 уровень: `animations: "disabled"` не всегда успевает довести CSS-переход до конца.
  use: { baseURL: "http://127.0.0.1:6007", reducedMotion: "reduce" },
  // threshold: 0 — по умолчанию 0.2, и тогда заметная смена фона проходит как «то же самое».
  expect: { toHaveScreenshot: { animations: "disabled", threshold: 0 } },
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
