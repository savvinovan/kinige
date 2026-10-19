import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Порт фиксирован: он указан в `devUrl` и в devCsp файла src-tauri/tauri.conf.json.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
