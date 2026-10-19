import { defineConfig, devices } from "@playwright/test";

// Снимки историй Storybook. Эталоны снимаются в Linux-контейнере (scripts/visual-tests.sh),
// тем же образом, что и в CI, поэтому `*-linux.png` — единственные эталоны в репозитории.
export default defineConfig({
  testDir: "tests/visual",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  reporter: [["list"]],
  use: { baseURL: "http://127.0.0.1:6007" },
  expect: { toHaveScreenshot: { animations: "disabled" } },
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
