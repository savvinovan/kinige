import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

// The focus ring only after a key press (feedback at the 4b acceptance): `useInputModality` sets `data-input="pointer"` on <html> after the mouse
// and while nothing has been pressed, `keyboard` after a key. One shared rule in `base.css` hides the ring in `pointer` mode. Any
// own ring of a component (`outline` in a rule with `:focus-visible`) must be in its list, otherwise it would appear after a
// click. Input fields have no ring: their focus is a thin line on `:focus` (the caret is visible), and it stays as is.

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
