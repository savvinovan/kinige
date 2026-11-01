import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { PALETTE_ROLES, TILE_ROLES, palettes } from "./tokenSource";

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
    } else if ((TILE_ROLES as readonly string[]).includes(role) && role !== "tile.neutral") {
      // Плитки подкрашены (D2): тон, насыщенность 30 % и светлота 31 % записаны в происхождении и совпадают с цветом.
      it(`${role}: авторская подкраска, не выданная за Nord, тон и светлота как в описании`, () => {
        expect(origin.startsWith(OWN), origin).toBe(true);
        expect(nord.map(([, value]) => value)).not.toContain(hex);
        const { hue, lightness } = hueAndLightness(hex);
        const stated = /тон (\d+)°, насыщенность 30 %, светлота 31 %/.exec(origin);
        expect(stated, origin).not.toBeNull();
        expect(Math.abs(hue - Number(stated?.[1]))).toBeLessThanOrEqual(2);
        expect(Math.abs(lightness - 31)).toBeLessThanOrEqual(1);
      });
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

/** Тон (градусы) и светлота HSL (проценты) цвета «#rrggbb». */
function hueAndLightness(hex: string): { hue: number; lightness: number } {
  const [r = 0, g = 0, b = 0] = [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16) / 255);
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const delta = max - min;
  const lightness = ((max + min) / 2) * 100;
  if (delta === 0) return { hue: 0, lightness };
  const sector = max === r ? ((g - b) / delta + 6) % 6 : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4;
  return { hue: sector * 60, lightness };
}
