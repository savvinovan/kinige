import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The port is fixed: it is set in `devUrl` and in devCsp of src-tauri/tauri.conf.json.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // fs.allow: web/ itself and design/tokens (tests and stories read the JSON tokens one level above web/).
  server: { port: 1420, strictPort: true, fs: { allow: [".", "../design/tokens"] } },
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
