import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Свойства, которые видны только в настоящем окне (jsdom CSS не считает): правила не должны пропасть из таблиц.
const css = (name: string) => readFileSync(new URL(name, import.meta.url), "utf8");

/** Тело правила с точным селектором (без комментариев внутри). */
const rule = (source: string, selector: string) => {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const body = new RegExp(`(?:^|\\})\\s*(?:/\\*[^]*?\\*/\\s*)*${escaped}\\s*\\{([^}]*)\\}`, "m").exec(source)?.[1];
  return (body ?? "").replace(/\/\*[^]*?\*\//g, "");
};

describe("Markdown: блок кода (KINIG-40)", () => {
  const pre = rule(css("./markdown.css"), ".markdown pre");

  it("правило блока найдено", () => {
    expect(pre).toContain("background: var(--color-surface-code)");
  });

  it("строки переносятся: горизонтальной прокрутки, а с ней и полосы под блоком, нет", () => {
    expect(pre).toMatch(/white-space:\s*pre-wrap/);
    expect(pre).toMatch(/overflow-wrap:\s*anywhere/);
  });

  it("у блока нет прокрутки и обрезки: полосе прокрутки не на чем лечь поверх следующего абзаца", () => {
    expect(pre).not.toMatch(/overflow(?:-x|-y)?\s*:/);
    expect(pre).not.toMatch(/(?:^|[\s;])(?:max-|min-)?height\s*:/);
    expect(pre).not.toMatch(/(?:line-clamp|clip|contain)\b/);
  });

  it("у вложенного кода нет собственной высоты, прокрутки и отрицательных полей", () => {
    const inner = rule(css("./markdown.css"), ".markdown pre code");
    expect(inner).not.toMatch(/(?:overflow|height|margin)/);
  });

  it("отступы между блоками положительные: ни одного отрицательного поля в оформлении Markdown", () => {
    expect(css("./markdown.css")).not.toMatch(/margin[a-z-]*:\s*-/);
    expect(css("./message.css")).not.toMatch(/margin[a-z-]*:\s*-/);
  });
});

describe("Тёмная схема системных элементов (KINIG-38)", () => {
  it("токен задаёт dark, base.css ставит его в color-scheme на html", () => {
    expect(css("../tokens.css")).toMatch(/--color-scheme:\s*dark;/);
    expect(rule(css("../base.css"), "html")).toMatch(/color-scheme:\s*var\(--color-scheme\)/);
  });
});
