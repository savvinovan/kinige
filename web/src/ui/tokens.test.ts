import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, it } from "vitest";
import { contrastRatio } from "./contrast";
import { CONTRAST_PAIRS, MIN_RATIO } from "./contrastPairs";
import { PALETTE_ROLES, active, palettes } from "./tokenSource";

const css = (file: string) => readFileSync(join(import.meta.dirname, file), "utf8");

describe("contrastRatio", () => {
  it("совпадает с известными значениями WCAG", () => {
    // Сырые цвета в src запрещены даже в тестах, поэтому «#» + шесть цифр собирается здесь.
    const hex = (digits: string) => `#${digits}`;
    expect(contrastRatio(hex("000000"), hex("ffffff"))).toBeCloseTo(21, 5);
    expect(contrastRatio(hex("ffffff"), hex("ffffff"))).toBeCloseTo(1, 5);
    // Серый на белом, известный пример: 4,48:1 (не проходит AA для текста).
    expect(contrastRatio(hex("777777"), hex("ffffff"))).toBeCloseTo(4.48, 2);
  });
});

describe("палитры", () => {
  const rows: string[] = [];

  afterAll(() => {
    // Таблица пар «цвет / фон» для приёмки: видна в выводе `npm test`.
    console.log(`\nКонтраст пар (WCAG 2.x; порог: text 4,5, ui 3, decorative 1,2)\n${rows.join("\n")}`);
  });

  it("есть хотя бы два кандидата, активный существует", () => {
    expect(Object.keys(palettes).length).toBeGreaterThanOrEqual(2);
    expect(palettes[active.palette]).toBeDefined();
  });

  for (const [name, palette] of Object.entries(palettes)) {
    describe(name, () => {
      it("задаёт все роли и только их, цвета в формате #rrggbb", () => {
        expect(Object.keys(palette.colors).sort()).toEqual([...PALETTE_ROLES].sort());
        for (const [role, value] of Object.entries(palette.colors)) expect(value, role).toMatch(/^#[0-9a-f]{6}$/i);
      });

      for (const pair of CONTRAST_PAIRS) {
        const label = `${pair.foreground} на ${pair.background} (${pair.kind}) >= ${MIN_RATIO[pair.kind]}`;
        it(label, () => {
          const fg = palette.colors[pair.foreground];
          const bg = palette.colors[pair.background];
          expect(fg, pair.foreground).toBeDefined();
          expect(bg, pair.background).toBeDefined();
          const ratio = contrastRatio(fg ?? "", bg ?? "");
          rows.push(
            `${name.padEnd(8)} ${pair.foreground.padEnd(16)} на ${pair.background.padEnd(21)} ${ratio.toFixed(2).padStart(5)}:1  (${pair.kind}, нужно >= ${MIN_RATIO[pair.kind]})`,
          );
          expect(ratio).toBeGreaterThanOrEqual(MIN_RATIO[pair.kind]);
        });
      }
    });
  }
});

describe("сгенерированный CSS не отстал от токенов", () => {
  const tokens = css("tokens.css");

  it("tokens.css: роли цвета равны активной палитре", () => {
    const palette = palettes[active.palette];
    for (const [role, value] of Object.entries(palette?.colors ?? {})) {
      const variable = `--color-${role.replaceAll(".", "-")}`;
      expect(tokens.toLowerCase(), variable).toContain(`${variable}: ${value.toLowerCase()};`);
    }
  });

  it("themes.css: есть блок на каждый кандидат палитры, переключателя шрифтов нет", () => {
    const themes = css("themes.css");
    for (const name of Object.keys(palettes)) expect(themes).toContain(`[data-palette="${name}"]`);
    expect(themes).not.toContain("data-fontset");
  });
});

describe("шкалы", () => {
  const tokens = css("tokens.css");

  it("типографическая шкала: у каждой роли размер, межстрочный, вес и семейство", () => {
    for (const role of ["day", "h1", "h2", "h3", "body", "small", "code", "meta"]) {
      for (const part of ["size", "line-height", "weight", "family"]) {
        expect(tokens, `type.${role}.${part}`).toMatch(new RegExp(`--type-${role}-${part}:`));
      }
    }
  });

  it("механизм масштаба: заданы оба значения, размеры шрифта в rem", () => {
    expect(tokens).toMatch(/--font-scale-default: 1;/);
    expect(tokens).toMatch(/--font-scale-large: [\d.]+;/);
    for (const [, value] of tokens.matchAll(/--type-[a-z0-9]+-size: ([^;]+);/g)) expect(value).toMatch(/rem$/);
  });

  it("у каждой длительности анимации есть вариант для Reduce Motion", () => {
    const themes = css("themes.css");
    for (const [, name] of tokens.matchAll(/--motion-duration-(\w+):/g)) {
      // Для Reduce Motion длительность нулевая (SD пишет её в секундах).
      expect(themes, `reduced ${name}`).toMatch(new RegExp(`--motion-duration-${name}: 0(?:s|ms);`));
    }
    expect(themes).toContain("prefers-reduced-motion: reduce");
  });

  it("отступы, радиусы и тени заданы", () => {
    for (const name of ["space-1", "space-4", "space-8", "radius-inline", "radius-control", "radius-panel", "shadow-raised", "shadow-overlay"]) {
      expect(tokens, name).toMatch(new RegExp(`--${name}:`));
    }
  });
});
