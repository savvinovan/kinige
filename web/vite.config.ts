import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The microphone source, the runners' probe (slice 7a, ADR-0015) and the screens driver. Read from `process.env` of this config, which Vite does not fill
// from `.env` files, so a stray `web/.env.local` cannot switch the fake microphone on. Without KINIGE_FAKE_MIC / KINIGE_PROBE the
// fake files (outside `web/src`) are not in the production module graph at all.
const fakeMic = process.env.KINIGE_FAKE_MIC === "1";
const probe = process.env.KINIGE_PROBE === "1";
// The screens driver (KINIG-92, ADR-0016): read the same way, and guarded the same way (scripts/check-test-hooks.sh).
const screensDriver = process.env.KINIGE_SCREENS_DRIVER === "1";
const path = (relative: string) => fileURLToPath(new URL(relative, import.meta.url));

// The port is fixed: it is set in `devUrl` and in devCsp of src-tauri/tauri.conf.json.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "virtual:kinige-mic": fakeMic ? path("../scripts/screens/fake-mic.ts") : path("./src/mic/real.ts"),
      "virtual:kinige-probe": probe ? path("../scripts/screens/probe.ts") : path("./src/mic/noProbe.ts"),
      "virtual:kinige-driver": screensDriver ? path("../scripts/screens/driver.ts") : path("./src/mic/noDriver.ts"),
    },
  },
  clearScreen: false,
  // fs.allow: web/ itself and design/tokens (tests and stories read the JSON tokens one level above web/).
  server: { port: 1420, strictPort: true, fs: { allow: [".", "../design/tokens", "../scripts/screens"] } },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
    // `npm run test:coverage` (CI → Codecov): app code only. Without tests, stories, generated types
    // and what only tests and Storybook need (MockApi and its data, the settings demo data, the stories frame).
    coverage: {
      provider: "v8",
      reporter: ["lcov", "text-summary"],
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.test.{ts,tsx}",
        "src/**/*.stories.tsx",
        "src/test/**",
        "src/api/generated/**",
        "src/api/MockApi*.ts",
        "src/api/mock*.ts",
        "src/api/demoData.ts",
        "src/ui/settings/demo*.ts",
        "src/ui/stories/**",
        "src/ui/emoji/EmojiStoryData.tsx",
      ],
    },
  },
});
