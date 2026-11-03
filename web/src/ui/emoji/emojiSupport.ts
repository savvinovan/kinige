import type { RawEmoji } from "./emojiData";

// Which emoji the system font draws. The emojibase data contains emoji of all Unicode Emoji versions, but the OS font does not know
// all of them: on an older system a new emoji is drawn as "tofu" (an empty box) or falls apart into pieces. The picker must not
// show what is not in the font.

export interface EmojiSupport {
  /** The highest Unicode Emoji version whose emoji the font draws; newer entries are hidden. */
  version: number;
  /** Whether country flags (🇪🇺) are drawn: on Windows they stay letters. */
  countryFlags: boolean;
}

/**
 * The version used when glyphs cannot be checked (no canvas: tests in jsdom). Not verified on a real system: Emoji 15.1 is
 * the last version that, from memory, macOS 14+ already draws; Emoji 16 and 17 are not within this limit until the glyph
 * check confirms them on the target system.
 */
export const FALLBACK_SUPPORT: EmojiSupport = { version: 15.1, countryFlags: true };

const SIZE = 2;
const EU_FLAG = "\u{1F1EA}\u{1F1FA}";

/**
 * A check by glyph, as frimousse (MIT) does it: the emoji is drawn on a canvas with two text colors. A color emoji does not depend on the text
 * color, while "tofu" and a fallback monochrome glyph do. A composite emoji that the font does not know falls apart into
 * several and becomes wider than two nominal units. `null` means the canvas is unavailable.
 */
function canvasContext(): CanvasRenderingContext2D | null {
  try {
    return document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  } catch {
    return null;
  }
}

function glyphRenderer(): ((emoji: string) => boolean) | null {
  const ctx = canvasContext();
  if (!ctx) return null;
  // The family comes from the token (--font-family-emoji), not from code: the only place where the design system sets it.
  const family = getComputedStyle(document.documentElement).getPropertyValue("--font-family-emoji").trim() || "sans-serif";
  ctx.canvas.width = SIZE;
  ctx.canvas.height = SIZE;
  return (emoji) => {
    ctx.font = `${SIZE}px ${family}`;
    ctx.textBaseline = "middle";
    if (ctx.measureText(emoji).width >= SIZE * 2) return false;
    const draw = (color: string) => {
      ctx.clearRect(0, 0, SIZE, SIZE);
      ctx.fillStyle = color;
      ctx.fillText(emoji, 0, 0);
      return ctx.getImageData(0, 0, SIZE, SIZE).data;
    };
    // Two arbitrary contrasting colors on an invisible canvas: these are probes, not interface colors.
    const first = draw("blue");
    const second = draw("red");
    for (let i = 0; i < SIZE * SIZE * 4; i += 4) {
      if (first[i] !== second[i] || first[i + 1] !== second[i + 1] || first[i + 2] !== second[i + 2]) return false;
    }
    return true;
  };
}

let detected: EmojiSupport | undefined;

/**
 * It looks for the limit by one emoji of each version, from new to old: the first version whose emoji is drawn is the limit.
 * The result is remembered for the session. Without a canvas, `FALLBACK_SUPPORT`. Not verified on a real macOS.
 */
export function detectSupport(emojis: readonly RawEmoji[]): EmojiSupport {
  if (detected) return detected;
  const renders = glyphRenderer();
  if (!renders) return FALLBACK_SUPPORT;
  const sample = new Map<number, string>();
  for (const emoji of emojis) {
    if (emoji.group !== undefined && !sample.has(emoji.version)) sample.set(emoji.version, emoji.emoji);
  }
  const descending = [...sample.keys()].sort((a, b) => b - a);
  const version = descending.find((candidate) => renders(sample.get(candidate) ?? "")) ?? descending[0] ?? FALLBACK_SUPPORT.version;
  detected = { version, countryFlags: renders(EU_FLAG) };
  return detected;
}
