import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Поведение, которое видно только в настоящем окне (jsdom CSS не считает): правила не должны пропасть из таблицы.
const sidebar = readFileSync(join(import.meta.dirname, "sidebar.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** Тело правила, в списке селекторов которого есть `selector`. */
function ruleFor(selector: string): string {
  const rules = [...sidebar.matchAll(/([^{}]+)\{([^}]*)\}/g)];
  const found = rules.find(([, selectors]) => (selectors ?? "").split(",").some((part) => part.trim() === selector));
  return found?.[2] ?? "";
}

describe("Сайдбар: правила CSS", () => {
  // Регрессия D2: `.chat-row:hover` (0,2,0) перебивал `.chat-row--drop` (0,1,0), и цель переноса под указателем получала
  // заливку наведения вместо заливки цели.
  for (const row of [".chat-row", ".rail-button"]) {
    it(`${row}: цель переноса под указателем сохраняет заливку цели, а не наведения`, () => {
      const drop = ruleFor(`${row}--drop:hover`);
      expect(drop).toContain("background: var(--color-surface-field-active)");
      expect(drop).toBe(ruleFor(`${row}--drop`));
      // Заливку наведения строка получает отдельным правилом, значит перебить его должен именно `--drop:hover`.
      expect(ruleFor(`${row}:hover`)).toContain("background: var(--color-surface-hover)");
    });
  }

  it("выбранная строка остаётся выбранной под указателем", () => {
    expect(ruleFor(".chat-row--selected:hover")).toContain("background: var(--color-surface-selected)");
    expect(ruleFor(".rail-button--selected:hover")).toContain("background: var(--color-surface-selected)");
  });
});
