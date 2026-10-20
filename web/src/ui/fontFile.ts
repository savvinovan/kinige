// Минимальный разбор woff2 для проверки встроенных шрифтов: кириллица (cmap), вес и курсив (OS/2, fvar), лицензия (name).
// Только чтение; нужные таблицы в woff2 не преобразуются (преобразуются только glyf, loca и, по желанию, hmtx).
// Формат: https://www.w3.org/TR/WOFF2/ ; таблицы sfnt: https://learn.microsoft.com/typography/opentype/spec/
import { brotliDecompressSync } from "node:zlib";

// Известные теги из спецификации WOFF2 (раздел 5.1), индекс = 6 младших бит флага.
const KNOWN_TAGS = [
  "cmap", "head", "hhea", "hmtx", "maxp", "name", "OS/2", "post", "cvt ", "fpgm", "glyf", "loca", "prep", "CFF ", "VORG",
  "EBDT", "EBLC", "gasp", "hdmx", "kern", "LTSH", "PCLT", "VDMX", "vhea", "vmtx", "BASE", "GDEF", "GPOS", "GSUB", "EBSC",
  "JSTF", "MATH", "CBDT", "CBLC", "COLR", "CPAL", "SVG ", "sbix", "acnt", "avar", "bdat", "bloc", "bsln", "cvar", "fdsc",
  "feat", "fmtx", "fvar", "gvar", "hsty", "just", "lcar", "mort", "morx", "opbd", "prop", "trak", "Zapf", "Silf", "Glat",
  "Gloc", "Feat", "Sill",
];

export interface FontInfo {
  tables: string[];
  /** Покрытые кодовые точки (из cmap). */
  has: (codePoint: number) => boolean;
  /** usWeightClass из OS/2. */
  weightClass: number;
  italic: boolean;
  /** Оси переменного шрифта: тег -> [min, default, max]; пусто у статических. */
  axes: Record<string, [number, number, number]>;
  /** Строка лицензии (name ID 13). */
  license: string;
}

function readBase128(view: DataView, start: number): [value: number, next: number] {
  let value = 0;
  for (let i = 0; i < 5; i++) {
    const byte = view.getUint8(start + i);
    value = value * 128 + (byte & 0x7f);
    if ((byte & 0x80) === 0) return [value, start + i + 1];
  }
  throw new Error("woff2: некорректное UIntBase128");
}

function tableDirectory(view: DataView): { tag: string; length: number }[] {
  if (view.getUint32(0) !== 0x774f4632) throw new Error("woff2: нет сигнатуры wOF2");
  const count = view.getUint16(12);
  let pos = 48;
  const tables: { tag: string; length: number }[] = [];
  for (let i = 0; i < count; i++) {
    const flags = view.getUint8(pos++);
    let tag = KNOWN_TAGS[flags & 0x3f] ?? "";
    if ((flags & 0x3f) === 0x3f) {
      tag = String.fromCharCode(...[0, 1, 2, 3].map((k) => view.getUint8(pos + k)));
      pos += 4;
    }
    const version = flags >> 6;
    let length: number;
    [length, pos] = readBase128(view, pos);
    const transformed = tag === "glyf" || tag === "loca" ? version !== 3 : version !== 0;
    if (transformed) [length, pos] = readBase128(view, pos);
    tables.push({ tag, length });
  }
  // Сжатый блок начинается сразу за каталогом таблиц.
  tables.push({ tag: "", length: pos });
  return tables;
}

function parseCmap(view: DataView, offset: number): (codePoint: number) => boolean {
  const ranges: [number, number][] = [];
  const subtables = view.getUint16(offset + 2);
  for (let i = 0; i < subtables; i++) {
    const sub = offset + view.getUint32(offset + 4 + i * 8 + 4);
    const format = view.getUint16(sub);
    if (format === 12) {
      const groups = view.getUint32(sub + 12);
      for (let g = 0; g < groups; g++) {
        const at = sub + 16 + g * 12;
        ranges.push([view.getUint32(at), view.getUint32(at + 4)]);
      }
    } else if (format === 4) {
      const segCount = view.getUint16(sub + 6) / 2;
      const ends = sub + 14;
      const starts = ends + segCount * 2 + 2;
      const deltas = starts + segCount * 2;
      const rangeOffsets = deltas + segCount * 2;
      for (let s = 0; s < segCount; s++) {
        const end = view.getUint16(ends + s * 2);
        const start = view.getUint16(starts + s * 2);
        const delta = view.getUint16(deltas + s * 2);
        const rangeOffset = view.getUint16(rangeOffsets + s * 2);
        for (let c = start; c <= end && c !== 0xffff; c++) {
          let glyph: number;
          if (rangeOffset === 0) glyph = (c + delta) & 0xffff;
          else {
            const raw = view.getUint16(rangeOffsets + s * 2 + rangeOffset + (c - start) * 2);
            glyph = raw === 0 ? 0 : (raw + delta) & 0xffff;
          }
          if (glyph !== 0) ranges.push([c, c]);
        }
      }
    }
  }
  return (codePoint) => ranges.some(([from, to]) => codePoint >= from && codePoint <= to);
}

function parseAxes(view: DataView, offset: number): FontInfo["axes"] {
  const axesOffset = view.getUint16(offset + 4);
  const axisCount = view.getUint16(offset + 8);
  const axisSize = view.getUint16(offset + 10);
  const axes: FontInfo["axes"] = {};
  for (let i = 0; i < axisCount; i++) {
    const at = offset + axesOffset + i * axisSize;
    const tag = String.fromCharCode(...[0, 1, 2, 3].map((k) => view.getUint8(at + k)));
    axes[tag] = [view.getInt32(at + 4) / 65536, view.getInt32(at + 8) / 65536, view.getInt32(at + 12) / 65536];
  }
  return axes;
}

function parseLicense(view: DataView, offset: number): string {
  const count = view.getUint16(offset + 2);
  const strings = offset + view.getUint16(offset + 4);
  for (let i = 0; i < count; i++) {
    const record = offset + 6 + i * 12;
    if (view.getUint16(record + 6) !== 13) continue;
    const platform = view.getUint16(record);
    const length = view.getUint16(record + 8);
    const start = strings + view.getUint16(record + 10);
    let text = "";
    if (platform === 1) for (let k = 0; k < length; k++) text += String.fromCharCode(view.getUint8(start + k));
    else for (let k = 0; k + 1 < length; k += 2) text += String.fromCharCode(view.getUint16(start + k));
    return text;
  }
  return "";
}

export function readFont(file: Uint8Array): FontInfo {
  const header = new DataView(file.buffer, file.byteOffset, file.byteLength);
  const directory = tableDirectory(header);
  const { length: dataStart } = directory.pop() ?? { length: 0 };
  const compressedSize = header.getUint32(20);
  const raw = brotliDecompressSync(file.subarray(dataStart, dataStart + compressedSize));
  const data = new DataView(raw.buffer, raw.byteOffset, raw.byteLength);

  const offsets = new Map<string, number>();
  let at = 0;
  for (const { tag, length } of directory) {
    offsets.set(tag, at);
    at += length;
  }
  const need = (tag: string): number => {
    const offset = offsets.get(tag);
    if (offset === undefined) throw new Error(`woff2: нет таблицы ${tag}`);
    return offset;
  };

  const os2 = need("OS/2");
  const fvar = offsets.get("fvar");
  return {
    tables: [...offsets.keys()],
    has: parseCmap(data, need("cmap")),
    weightClass: data.getUint16(os2 + 4),
    italic: (data.getUint16(os2 + 62) & 1) === 1,
    axes: fvar === undefined ? {} : parseAxes(data, fvar),
    license: parseLicense(data, need("name")),
  };
}
