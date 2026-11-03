import { defineConfig, devices } from "@playwright/test";

// Storybook story snapshots. The baselines are captured on GitHub (.github/workflows/visual-update.yml)
// the same way as in CI, so `*-linux.png` are the only baselines in the repository.
export default defineConfig({
  testDir: "tests/visual",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  // One retry in CI for one-off load hiccups; it does not cure the 2-pixel difference (see NOISY in tests/visual/stories.spec.ts).
  retries: process.env.CI ? 1 : 0,
  reporter: [["list"]],
  // reducedMotion: the design system transitions are zero under Reduce Motion (themes.css), so a snapshot cannot catch them midway.
  // Without it a focused input field (a fill change + the focus line at a rounded corner) gave a 2-pixel difference in ~1% of runs
  // at a ±1 level: `animations: "disabled"` does not always manage to finish a CSS transition.
  use: { baseURL: "http://127.0.0.1:6007", reducedMotion: "reduce" },
  // threshold: 0 - the default is 0.2, and then a noticeable background change passes as "the same".
  // No pixel tolerance here: the two stories with a known 2-pixel rasterization noise get one in tests/visual/stories.spec.ts.
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
