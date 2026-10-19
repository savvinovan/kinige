import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

// Цвета, размеры и шрифты живут только в design/tokens -> tokens.css (§11.1 ТЗ).
const SRC = join(import.meta.dirname, "..");
const SKIP = new Set(["ui/tokens.css", "api/generated"]);

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    const rel = relative(SRC, path);
    if (SKIP.has(rel)) return [];
    if (statSync(path).isDirectory()) return files(path);
    return /\.(css|tsx?)$/.test(name) && !name.endsWith("no-raw-values.test.ts") ? [path] : [];
  });
}

const FORBIDDEN: [string, RegExp][] = [
  ["hex-цвет", /#[0-9a-fA-F]{3,8}\b/],
  ["функция цвета", /\b(rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch|color-mix)\(/],
  ["размер в px/rem/em/pt", /\b\d*\.?\d+(px|rem|em|pt)\b/],
  ["font-family вне var()", /font-family\s*:\s*(?!var\()\S/],
  ["font-size вне var()", /font-size\s*:\s*(?!var\()\S/],
];

describe("сырые значения вне tokens.css", () => {
  for (const file of files(SRC)) {
    it(relative(SRC, file), () => {
      const text = readFileSync(file, "utf8");
      for (const [name, pattern] of FORBIDDEN) {
        expect(text, name).not.toMatch(pattern);
      }
    });
  }
});
