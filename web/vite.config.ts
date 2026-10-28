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
    // `npm run test:coverage` (CI → Codecov): только код приложения, без тестов, историй и сгенерированных типов.
    coverage: {
      provider: "v8",
      reporter: ["lcov", "text-summary"],
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/**/*.test.{ts,tsx}", "src/**/*.stories.tsx", "src/test/**", "src/api/generated/**"],
    },
  },
});
