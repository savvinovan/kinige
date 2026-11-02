import { defineConfig } from "vitest/config";

// The page imports the design system tokens from ../../web/src/ui, hence the fs.allow.
export default defineConfig({
  base: "./",
  server: { fs: { allow: ["../.."] } },
  test: { environment: "node", include: ["src/**/*.test.ts"] },
});
