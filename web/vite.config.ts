import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Порт фиксирован: он указан в `devUrl` и в devCsp файла src-tauri/tauri.conf.json.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // fs.allow: сам web/ и design/tokens (тесты и истории читают токены JSON на уровень выше web/).
  server: { port: 1420, strictPort: true, fs: { allow: [".", "../design/tokens"] } },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
    // `npm run test:coverage` (CI → Codecov): только код приложения. Без тестов, историй, сгенерированных типов
    // и того, что нужно только тестам и Storybook (MockApi и его данные, демо-данные настроек, рамка историй).
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
