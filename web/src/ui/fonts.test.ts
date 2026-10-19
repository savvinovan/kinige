import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { describe, expect, it } from "vitest";
import { type FontInfo, readFont } from "./fontFile";
import { type FontFace, active, fontsets } from "./tokenSource";

// Шрифты проверяются по самим файлам: кириллица (cmap), курсив (OS/2), вес (OS/2 или ось wght), лицензия (name).
// Описание начертания в токенах — заявка, а тест сверяет её с файлом.

const UI_DIR = import.meta.dirname;

/** Начертания, которые нужны любому набору: роль, стиль, вес. Остальное (курсив sans и mono) не используется. */
const REQUIRED: [role: string, style: "normal" | "italic", weight: number][] = [
  ["serif", "normal", 400],
  ["serif", "italic", 400],
  ["serif", "normal", 600],
  ["serif", "normal", 700],
  ["sans", "normal", 400],
  ["sans", "normal", 600],
  ["sans", "normal", 700],
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

/** Вес, который реально покрывает файл: диапазон оси wght или единственное значение OS/2. */
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
  it("есть хотя бы два кандидата и активный набор существует", () => {
    expect(Object.keys(fontsets).length).toBeGreaterThanOrEqual(2);
    expect(fontsets[active.fontset]).toBeDefined();
  });

  for (const [name, set] of Object.entries(fontsets)) {
    describe(name, () => {
      it("задаёт семейства serif, sans, mono, и первое семейство каждой роли есть среди начертаний", () => {
        for (const role of ["serif", "sans", "mono"]) {
          const first = set.families[role]?.[0];
          expect(first, `family.${role}`).toBeTruthy();
          const faces = Object.entries(set.faces).filter(([key]) => key.startsWith(`${role}-`));
          expect(faces.length, `начертания роли ${role}`).toBeGreaterThan(0);
          for (const [key, face] of faces) expect(face.family, key).toBe(first);
        }
      });

      for (const [key, face] of Object.entries(set.faces)) {
        describe(key, () => {
          it("файл woff2 существует, рядом лежит OFL.txt", () => {
            expect(face.file).toMatch(/\.woff2$/);
            expect(existsSync(join(UI_DIR, face.file)), face.file).toBe(true);
            expect(existsSync(join(dirname(join(UI_DIR, face.file)), "OFL.txt")), "OFL.txt рядом").toBe(true);
          });

          it("содержит русский алфавит (А–я, Ё, ё) и лицензия внутри файла — OFL", () => {
            const info = readFace(face);
            const missing = RUSSIAN.filter((codePoint) => !info.has(codePoint));
            expect(missing.map((c) => c.toString(16)), "нет кодовых точек").toEqual([]);
            const block = Array.from({ length: 0x100 }, (_, i) => 0x400 + i).filter((c) => info.has(c));
            expect(block.length, "кодовых точек в U+0400–U+04FF").toBeGreaterThanOrEqual(90);
            expect(info.license).toContain("Open Font License");
          });

          it("стиль и вес в файле совпадают с заявленными в токене", () => {
            const info = readFace(face);
            expect(info.italic, "курсив в OS/2").toBe(face.style === "italic");
            const [from, to] = parseWeight(face.weight);
            const [min, max] = fileWeights(info);
            expect([from, to], `заявлено ${face.weight}, в файле ${min}–${max}`).toEqual(
              info.axes.wght ? [min, max] : [min, min],
            );
          });
        });
      }

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
