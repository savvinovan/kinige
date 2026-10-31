import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

// Кольцо фокуса только после клавиши (отзыв на приёмке 4b): `useInputModality` ставит `data-input="pointer"` на <html> после мыши
// и пока ничего не нажато, `keyboard` после клавиши. Одно общее правило в `base.css` гасит кольцо в режиме `pointer`. Любое
// собственное кольцо компонента (`outline` в правиле с `:focus-visible`) должно быть в его списке, иначе оно появится после
// щелчка. Поля ввода кольца не имеют: их фокус — тонкая линия по `:focus` (каретка видна), она остаётся как есть.

const ROOT = import.meta.dirname;
const strip = (source: string) => source.replace(/\/\*[\s\S]*?\*\//g, "");

function cssFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return cssFiles(path);
    return name.endsWith(".css") &&
      name !== "tokens.css" &&
      name !== "themes.css"
      ? [path]
      : [];
  });
}

function rules(source: string): [string, string][] {
  return [...strip(source).matchAll(/([^{}]+)\{([^{}]*)\}/g)].flatMap(
    ([, selector = "", body = ""]) =>
      selector
        .split(",")
        .map((one): [string, string] => [
          one.trim().replace(/\s+/g, " "),
          body,
        ]),
  );
}

const DRAWS_OUTLINE = /(^|[;\s])outline\s*:(?!\s*none)/;
const BASE = 'html[data-input="pointer"]';
const quiet = new Set(
  rules(readFileSync(join(ROOT, "base.css"), "utf8"))
    .filter(([, body]) => /outline\s*:\s*none/.test(body))
    .map(([selector]) => selector),
);

describe("кольцо фокуса: только в режиме клавиатуры", () => {
  it("общее правило гасит `:focus-visible` в режиме pointer", () => {
    expect(quiet.has(`${BASE} :focus-visible`)).toBe(true);
  });

  it("каждое собственное кольцо компонента на :focus-visible есть в списке base.css (или привязано к data-input=keyboard)", () => {
    const missing: string[] = [];
    for (const file of cssFiles(ROOT)) {
      if (file.endsWith("base.css")) continue;
      for (const [selector, body] of rules(readFileSync(file, "utf8"))) {
        if (!selector.includes(":focus-visible") || !DRAWS_OUTLINE.test(body))
          continue;
        if (selector.startsWith('html[data-input="keyboard"]')) continue;
        if (!quiet.has(`${BASE} ${selector}`))
          missing.push(`${relative(ROOT, file)}: ${selector}`);
      }
    }
    expect(missing).toEqual([]);
  });

  it("поля ввода: фокус при вводе (`:focus`) не зависит от режима и не гасится", () => {
    for (const selector of quiet)
      expect(selector).not.toMatch(/:focus(?!-visible)/);
  });
});
