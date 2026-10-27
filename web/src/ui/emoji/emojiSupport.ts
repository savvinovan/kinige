import type { RawEmoji } from "./emojiData";

// Какие эмодзи рисует системный шрифт. Данные emojibase содержат эмодзи всех версий Unicode Emoji, а шрифт ОС знает не
// все: на более старой системе новый эмодзи рисуется «тофу» (пустой рамкой) или распадается на части. Пикер не должен
// показывать то, чего нет в шрифте.

export interface EmojiSupport {
  /** Наибольшая версия Unicode Emoji, чей эмодзи шрифт рисует; записи новее скрываются. */
  version: number;
  /** Рисуются ли флаги стран (🇪🇺): на Windows они остаются буквами. */
  countryFlags: boolean;
}

/**
 * Версия, когда проверить глифы нельзя (нет canvas: тесты в jsdom). Не проверено на реальной системе: Emoji 15.1 —
 * последняя версия, которую, по памяти, уже рисует macOS 14+; Emoji 16 и 17 в эту границу не входят, пока проверка
 * по глифам не подтвердит их на целевой системе.
 */
export const FALLBACK_SUPPORT: EmojiSupport = { version: 15.1, countryFlags: true };

const SIZE = 2;
const EU_FLAG = "\u{1F1EA}\u{1F1FA}";

/**
 * Проверка по глифу, как делает frimousse (MIT): эмодзи рисуется на canvas двумя цветами текста. Цветной эмодзи от цвета
 * текста не зависит, а «тофу» и запасной монохромный глиф зависят. Составной эмодзи, которого шрифт не знает, распадается на
 * несколько и становится шире двух условных единиц. `null` — canvas недоступен.
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
  // Семейство берётся из токена (--font-family-emoji), а не из кода: единственное место, где его задаёт дизайн-система.
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
    // Два произвольных контрастных цвета на невидимом canvas: это пробы, а не цвета интерфейса.
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
 * Границу ищет по одному эмодзи каждой версии, от новой к старой: первая версия, чей эмодзи нарисован, и есть предел.
 * Результат запоминается на сессию. Без canvas — `FALLBACK_SUPPORT`. На реальном macOS не проверено.
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
