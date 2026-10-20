// Контраст по WCAG 2.x: https://www.w3.org/TR/WCAG22/#dfn-contrast-ratio

function channel(value: number): number {
  const c = value / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** Относительная яркость цвета «#rrggbb» (WCAG). */
export function relativeLuminance(hex: string): number {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!match) throw new Error(`Ожидался цвет #rrggbb, получено: ${hex}`);
  const [r, g, b] = [match[1], match[2], match[3]].map((part) => channel(parseInt(part ?? "", 16)));
  return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
}

/** Коэффициент контраста двух цветов, от 1 до 21. */
export function contrastRatio(a: string, b: string): number {
  const [light, dark] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x);
  return ((light ?? 0) + 0.05) / ((dark ?? 0) + 0.05);
}
