import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { PALETTE_ROLES, palettes } from "./tokenSource";

// Происхождение цветов палитры размечено честно: у каждой роли указано, откуда цвет.
//  - «авторский: …» — наш собственный нейтральный серый (поверхности, текст, разделитель); в Nord таких нет;
//  - официальное имя Nord («nord8 …») — цвет обязан совпасть с первоисточником (design/sources/official-palettes.json)
//    и по значению, и по имени;
//  - «подгонка: …» — цвет выведен из официального, но обязан не совпадать ни с одним официальным.
// Так «настоящий Nord» нельзя подменить незаметно, а авторский цвет нельзя выдать за официальный.

const OFFICIAL = JSON.parse(readFileSync(join(import.meta.dirname, "../../../design/sources/official-palettes.json"), "utf8")) as Record<
  string,
  { source: string; colors: Record<string, string> }
>;

const FITTED = "подгонка";
const OWN = "авторский";
const nord = Object.entries(OFFICIAL.nord?.colors ?? {});
const [name, palette] = Object.entries(palettes)[0] ?? [];

/** Роли, которые обязаны быть от Nord: акцент, кольцо фокуса и ошибка. Остальное — авторское. */
const FROM_NORD = ["accent.default", "focus.ring", "status.error"];

describe("происхождение цветов палитры graphite", () => {
  it("палитра одна, у неё есть первоисточник Nord для акцента и ошибки", () => {
    expect(name).toBe("graphite");
    expect(OFFICIAL.nord?.source).toMatch(/^https:\/\//);
    expect(nord.map(([key]) => key).sort()).toEqual(["nord11", "nord8"]);
  });

  it("происхождение указано у каждой роли", () => {
    for (const role of PALETTE_ROLES) expect(palette?.origins[role], role).toBeTruthy();
  });

  for (const role of PALETTE_ROLES) {
    const origin = palette?.origins[role] ?? "";
    const hex = (palette?.colors[role] ?? "").toLowerCase();

    if (FROM_NORD.includes(role)) {
      if (origin.startsWith(FITTED)) {
        it(`${role}: подгонка из Nord не совпадает ни с одним официальным цветом`, () => {
          expect(nord.map(([, value]) => value)).not.toContain(hex);
        });
      } else {
        it(`${role}: указанное имя и значение совпадают с официальным цветом Nord`, () => {
          const match = nord.find(([, value]) => value === hex);
          expect(match, `${hex} нет среди официальных цветов`).toBeDefined();
          expect(origin.startsWith(`${match?.[0]} `), `«${origin}» должно начинаться с «${match?.[0]} »`).toBe(true);
        });
      }
    } else {
      it(`${role}: авторский нейтральный серый (насыщенность 0), не выданный за Nord`, () => {
        expect(origin.startsWith(OWN), origin).toBe(true);
        expect(nord.map(([, value]) => value)).not.toContain(hex);
        // Нейтральный серый: все три канала равны.
        expect(hex, role).toMatch(/^#([0-9a-f]{2})\1\1$/);
      });
    }
  }
});
