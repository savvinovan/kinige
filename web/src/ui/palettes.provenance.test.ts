import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { PALETTE_ROLES, TILE_ROLES, palettes } from "./tokenSource";

// The origin of the palette colors is marked honestly: every role says where its color comes from.
//  - "авторский: ..." (own) - our own neutral gray (surfaces, text, divider); Nord has none of these;
//  - an official Nord name ("nord8 ...") - the color must match the primary source (design/sources/official-palettes.json)
//    both by value and by name;
//  - "подгонка: ..." (fitted) - the color is derived from an official one but must not equal any official color.
// This way the "real Nord" cannot be swapped quietly, and an own color cannot be passed off as official.

const OFFICIAL = JSON.parse(readFileSync(join(import.meta.dirname, "../../../design/sources/official-palettes.json"), "utf8")) as Record<
  string,
  { source: string; colors: Record<string, string> }
>;

const FITTED = "подгонка";
const OWN = "авторский";
const nord = Object.entries(OFFICIAL.nord?.colors ?? {});
const [name, palette] = Object.entries(palettes)[0] ?? [];

/** Roles that must come from Nord: accent, focus ring and error. The rest is our own. */
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
          expect(match, `${hex} is not among the official colors`).toBeDefined();
          expect(origin.startsWith(`${match?.[0]} `), `"${origin}" must start with "${match?.[0]} "`).toBe(true);
        });
      }
    } else if ((TILE_ROLES as readonly string[]).includes(role) || role === "status.recording") {
      // The tiles are tinted (D2), and so is the recording dot (slice 7a): hue, saturation and lightness are recorded in the origin and match
      // the color. (The unplayed bars of a waveform are a neutral gray and take the branch below.)
      it(`${role}: авторская подкраска, не выданная за Nord, тон, насыщенность и светлота как в описании`, () => {
        expect(origin.startsWith(OWN), origin).toBe(true);
        expect(nord.map(([, value]) => value)).not.toContain(hex);
        const { hue, saturation, lightness } = toHueSaturationLightness(hex);
        const stated = /тон (\d+)°, насыщенность (\d+) %, светлота (\d+) %/.exec(origin);
        expect(stated, origin).not.toBeNull();
        expect(Math.abs(hue - Number(stated?.[1]))).toBeLessThanOrEqual(2);
        expect(Math.abs(saturation - Number(stated?.[2]))).toBeLessThanOrEqual(1.5);
        expect(Math.abs(lightness - Number(stated?.[3]))).toBeLessThanOrEqual(1);
      });
    } else {
      it(`${role}: авторский нейтральный серый (насыщенность 0), не выданный за Nord`, () => {
        expect(origin.startsWith(OWN), origin).toBe(true);
        expect(nord.map(([, value]) => value)).not.toContain(hex);
        // Neutral gray: all three channels are equal.
        expect(hex, role).toMatch(/^#([0-9a-f]{2})\1\1$/);
      });
    }
  }
});

/** Hue (degrees), saturation and lightness of HSL (percent) of a "#rrggbb" color. */
function toHueSaturationLightness(hex: string): { hue: number; saturation: number; lightness: number } {
  const [r = 0, g = 0, b = 0] = [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16) / 255);
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const delta = max - min;
  const lightness = (max + min) / 2;
  if (delta === 0) return { hue: 0, saturation: 0, lightness: lightness * 100 };
  const saturation = delta / (1 - Math.abs(2 * lightness - 1));
  const sector = max === r ? ((g - b) / delta + 6) % 6 : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4;
  return { hue: sector * 60, saturation: saturation * 100, lightness: lightness * 100 };
}
