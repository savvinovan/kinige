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
  },
});
