import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Behavior visible only in a real window (jsdom does not compute CSS): the rules must not vanish from the stylesheet.
const sidebar = readFileSync(join(import.meta.dirname, "sidebar.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** The body of the rule whose selector list contains `selector`. */
function ruleFor(selector: string): string {
  const rules = [...sidebar.matchAll(/([^{}]+)\{([^}]*)\}/g)];
  const found = rules.find(([, selectors]) => (selectors ?? "").split(",").some((part) => part.trim() === selector));
  return found?.[2] ?? "";
}

describe("Сайдбар: правила CSS", () => {
  // D2 regression: `.chat-row:hover` (0,2,0) overrode `.chat-row--drop` (0,1,0), and the drop target under the pointer got
  // the hover fill instead of the target fill.
  for (const row of [".chat-row", ".rail-button"]) {
    it(`${row}: цель переноса под указателем сохраняет заливку цели, а не наведения`, () => {
      const drop = ruleFor(`${row}--drop:hover`);
      expect(drop).toContain("background: var(--color-surface-field-active)");
      expect(drop).toBe(ruleFor(`${row}--drop`));
      // The row gets the hover fill from a separate rule, so it is `--drop:hover` that must override it.
      expect(ruleFor(`${row}:hover`)).toContain("background: var(--color-surface-hover)");
    });
  }

  it("выбранная строка остаётся выбранной под указателем", () => {
    expect(ruleFor(".chat-row--selected:hover")).toContain("background: var(--color-surface-selected)");
    expect(ruleFor(".rail-button--selected:hover")).toContain("background: var(--color-surface-selected)");
  });
});
