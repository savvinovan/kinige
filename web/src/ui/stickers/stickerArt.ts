// Synthetic stickers for stories, tests and MockApi (rules 9 and 10: no real sticker, no Telegram art): a rounded blob in two palette colours
// with the sticker's emoji on it, drawn as an SVG data URL. The colours are read from the design system tokens when the picture is made.

const PALETTE = [
  ["--color-tile-blue", "--color-accent-default"],
  ["--color-tile-green", "--color-forest-summer-crown-light"],
  ["--color-tile-orange", "--color-forest-autumn-accent"],
] as const;

const cache = new Map<string, string>();
/** The library stickers the demo pictures know: library id -> its emoji (the picture behind `kinige-media://localhost/sticker/<id>`). */
const library = new Map<string, string | null>();

/** Tells the demo media source about a library sticker, so `demoMediaSource("sticker", id)` can draw it. */
export function registerDemoSticker(id: string, emoji: string | null): void {
  library.set(id, emoji);
}

/** The picture of a registered library sticker, or `null` for an id nobody registered. */
export function libraryStickerUrl(id: string): string | null {
  return library.has(id) ? stickerArt(library.get(id) ?? null, id) : null;
}

function token(name: string): string {
  return typeof document === "undefined" ? "" : getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** A sticker picture as an SVG data URL; the same seed gives the same colours. */
export function stickerArt(emoji: string | null, seed: string, width = 512, height = 512): string {
  const key = `${emoji ?? ""}|${seed}|${width}x${height}`;
  const cached = cache.get(key);
  if (cached) return cached;
  const [from, to] = PALETTE[Array.from(seed).reduce((sum, symbol) => sum + symbol.charCodeAt(0), 0) % PALETTE.length] ?? PALETTE[0];
  const radius = Math.round(Math.min(width, height) / 5);
  const glyph = Math.round(Math.min(width, height) / 2);
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${token(from)}"/><stop offset="1" stop-color="${token(to)}"/></linearGradient></defs>` +
    `<rect x="${radius / 2}" y="${radius / 2}" width="${width - radius}" height="${height - radius}" rx="${radius}" fill="url(#g)"/>` +
    `<text x="50%" y="52%" text-anchor="middle" dominant-baseline="central" font-family="sans-serif" font-size="${glyph}">${emoji ?? ""}</text>` +
    `</svg>`;
  const url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  cache.set(key, url);
  return url;
}
