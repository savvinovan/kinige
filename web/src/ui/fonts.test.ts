import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, expect, it } from "vitest";
import { type FontInfo, readFont } from "./fontFile";
import { type FontFace, active, fontsets } from "./tokenSource";

// Fonts are checked against the files themselves: Cyrillic (cmap), italic (OS/2), weight (OS/2 or the wght axis), license (name).
// The face description in the tokens is a claim, and the test checks it against the file.

const UI_DIR = import.meta.dirname;

/** Faces that every set needs: role, style, weight. The rest (mono italic) is not used. */
const REQUIRED: [role: string, style: "normal" | "italic", weight: number][] = [
  ["sans", "normal", 400],
  ["sans", "italic", 400],
  ["sans", "normal", 600],
  ["sans", "normal", 700],
  ["sans", "italic", 700],
  ["mono", "normal", 400],
  ["mono", "normal", 700],
];

const parseWeight = (weight: string): [number, number] => {
  const [from = "", to = from] = weight.split(" ");
  return [Number(from), Number(to)];
};

const fontCache = new Map<string, FontInfo>();
const readFace = (face: FontFace): FontInfo => {
  let info = fontCache.get(face.file);
  if (!info) {
    info = readFont(new Uint8Array(readFileSync(join(UI_DIR, face.file))));
    fontCache.set(face.file, info);
  }
  return info;
};

/** The weight the file actually covers: the range of the wght axis or the single OS/2 value. */
const fileWeights = (info: FontInfo): [number, number] => {
  const axis = info.axes.wght;
  return axis ? [axis[0], axis[2]] : [info.weightClass, info.weightClass];
};

const RUSSIAN = [
  ...Array.from({ length: 0x44f - 0x410 + 1 }, (_, i) => 0x410 + i),
  0x401, // Ё
  0x451, // ё
];

describe("наборы шрифтов", () => {
  it("активный набор существует", () => {
    expect(Object.keys(fontsets).length).toBeGreaterThanOrEqual(1);
    expect(fontsets[active.fontset]).toBeDefined();
  });

  for (const [name, set] of Object.entries(fontsets)) {
    describe(name, () => {
      it("задаёт семейства sans, mono, и первое семейство каждой роли есть среди начертаний", () => {
        for (const role of ["sans", "mono"]) {
          const first = set.families[role]?.[0];
          expect(first, `family.${role}`).toBeTruthy();
          const faces = Object.entries(set.faces).filter(([key]) => key.startsWith(`${role}-`));
          expect(faces.length, `faces of role ${role}`).toBeGreaterThan(0);
          for (const [key, face] of faces) expect(face.family, key).toBe(first);
        }
      });

      for (const [key, face] of Object.entries(set.faces)) {
        describe(key, () => {
          it("файл woff2 существует, рядом лежит OFL.txt", () => {
            expect(face.file).toMatch(/\.woff2$/);
            expect(existsSync(join(UI_DIR, face.file)), face.file).toBe(true);
            expect(existsSync(join(dirname(join(UI_DIR, face.file)), "OFL.txt")), "OFL.txt next to it").toBe(true);
          });

          it("содержит русский алфавит (А–я, Ё, ё) и лицензия внутри файла — OFL", () => {
            const info = readFace(face);
            const missing = RUSSIAN.filter((codePoint) => !info.has(codePoint));
            expect(missing.map((c) => c.toString(16)), "missing code points").toEqual([]);
            const block = Array.from({ length: 0x100 }, (_, i) => 0x400 + i).filter((c) => info.has(c));
            expect(block.length, "code points in U+0400-U+04FF").toBeGreaterThanOrEqual(90);
            expect(info.license).toContain("Open Font License");
          });

          it("стиль и вес в файле совпадают с заявленными в токене", () => {
            const info = readFace(face);
            expect(info.italic, "italic in OS/2").toBe(face.style === "italic");
            const [from, to] = parseWeight(face.weight);
            const [min, max] = fileWeights(info);
            expect([from, to], `claimed ${face.weight}, in the file ${min}-${max}`).toEqual(
              info.axes.wght ? [min, max] : [min, min],
            );
          });
        });
      }

      // The only exception to "fonts are embedded only" (ADR-0004): chat emoji are user data,
      // and color emoji are not embedded. Any other role without a file in the set is an error.
      it("кроме эмодзи, у каждой роли есть встроенные начертания; эмодзи — системные шрифты", () => {
        expect(Object.keys(set.families).sort()).toEqual(["emoji", "mono", "sans"]);
        expect(Object.keys(set.faces).filter((key) => key.startsWith("emoji-"))).toEqual([]);
        const embedded = new Set(Object.values(set.faces).map((face) => face.family));
        for (const family of set.families.emoji ?? []) expect(embedded.has(family), family).toBe(false);
        expect(set.families.emoji?.length ?? 0).toBeGreaterThan(0);
      });

      it("покрывает все нужные начертания: regular, italic, semibold, bold", () => {
        for (const [role, style, weight] of REQUIRED) {
          const covering = Object.entries(set.faces).filter(([key, face]) => {
            if (!key.startsWith(`${role}-`) || face.style !== style) return false;
            const [min, max] = fileWeights(readFace(face));
            return min <= weight && weight <= max;
          });
          expect(covering.length, `${role} ${style} ${weight}`).toBeGreaterThan(0);
        }
      });
    });
  }
});
