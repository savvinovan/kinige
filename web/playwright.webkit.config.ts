import { defineConfig, devices } from "@playwright/test";

// Только для проверки прокрутки в окне «Доступ ИИ-агентов» настоящим WebKit (движок webview Tauri на macOS): запускается
// на macOS-раннере (`.github/workflows/webkit-scrollers.yml`). Снимков не сравнивает.
export default defineConfig({
  testDir: "tests/visual",
  testMatch: "scrollbars.spec.ts",
  retries: 0,
  reporter: [["list"]],
  use: { baseURL: "http://127.0.0.1:6007", reducedMotion: "reduce" },
  projects: [{ name: "webkit", use: { ...devices["Desktop Safari"], viewport: { width: 1024, height: 681 } } }],
  webServer: {
    command: "npx http-server storybook-static --port 6007 --address 127.0.0.1 --silent",
    url: "http://127.0.0.1:6007/index.json",
    reuseExistingServer: false,
  },
});
