import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { PALETTE_ROLES, palettes } from "./tokenSource";

// Палитра по мотивам готовой схемы (Nord) честно размечена: у каждой роли указано происхождение.
// «Официальный» цвет обязан совпасть с первоисточником (design/sources/official-palettes.json) и по значению,
// и по имени; всё остальное обязано называться подгонкой. Так «настоящий Nord» нельзя подменить незаметно.

const OFFICIAL = JSON.parse(readFileSync(join(import.meta.dirname, "../../../design/sources/official-palettes.json"), "utf8")) as Record<
  string,
  { source: string; colors: Record<string, string> }
>;

const FITTED = "подгонка";
const schemes = Object.entries(palettes).filter(([, palette]) => Object.keys(palette.origins).length > 0);

describe("происхождение цветов палитры по мотивам готовой схемы", () => {
  it("палитра есть и у неё есть официальный первоисточник", () => {
    expect(schemes.map(([name]) => name)).toEqual(["nord"]);
    for (const [name] of schemes) expect(OFFICIAL[name]?.source, name).toMatch(/^https:\/\//);
  });

  for (const [name, palette] of schemes) {
    describe(name, () => {
      const official = Object.entries(OFFICIAL[name]?.colors ?? {});

      it("происхождение указано у каждой роли", () => {
        for (const role of PALETTE_ROLES) expect(palette.origins[role], role).toBeTruthy();
      });

      for (const role of PALETTE_ROLES) {
        const origin = palette.origins[role] ?? "";
        const hex = (palette.colors[role] ?? "").toLowerCase();
        if (origin.startsWith(FITTED)) {
          it(`${role}: подгонка не совпадает ни с одним официальным цветом`, () => {
            expect(official.map(([, value]) => value)).not.toContain(hex);
          });
        } else {
          it(`${role}: указанное имя и значение совпадают с официальной палитрой`, () => {
            const match = official.find(([, value]) => value === hex);
            expect(match, `${hex} нет среди официальных цветов`).toBeDefined();
            expect(origin.startsWith(`${match?.[0]} `), `«${origin}» должно начинаться с «${match?.[0]} »`).toBe(true);
          });
        }
      }
    });
  }
});
