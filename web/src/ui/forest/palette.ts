import type { Season } from "../../api/generated/Season";

// The colours of the board, read from the design tokens (`color.forest.*`, `opacity.forest.*`) as CSS variables. Nothing here
// is a literal colour: slice 9 may retune the tokens and the board follows.

export interface SeasonColors {
  groundA: string;
  groundB: string;
  crownMid: string;
  crownDark: string;
  crownLight: string;
  trunk: string;
  accent: string;
}

export interface Palette {
  seasons: Record<Season, SeasonColors>;
  /** Ground of a cell without a date. */
  voidTile: string;
  shadow: string;
  outline: string;
  outlineToday: string;
  outlineHover: string;
  label: string;
  labelHalo: string;
  /** Canvas background: the paper of the feed. */
  background: string;
  /** Opacity of a tree in front of the selected one. */
  frontAlpha: number;
  /** Opacity of the fill under the selected cell outline. */
  selectionFill: number;
  /** Canvas font for labels, read from the canvas element (type tokens, so large text scales it). */
  font: string;
}

export const SEASONS: readonly Season[] = ["winter", "spring", "summer", "autumn"];

export function readPalette(canvas: HTMLElement): Palette {
  const style = getComputedStyle(canvas);
  const root = getComputedStyle(document.documentElement);
  const read = (name: string) => root.getPropertyValue(name).trim();
  // A missing token reads as fully opaque.
  const number = (name: string) => {
    const value = Number.parseFloat(read(name));
    return Number.isFinite(value) ? value : 1;
  };
  const season = (name: Season): SeasonColors => ({
    groundA: read(`--color-forest-${name}-ground-a`),
    groundB: read(`--color-forest-${name}-ground-b`),
    crownMid: read(`--color-forest-${name}-crown-mid`),
    crownDark: read(`--color-forest-${name}-crown-dark`),
    crownLight: read(`--color-forest-${name}-crown-light`),
    trunk: read(`--color-forest-${name}-trunk`),
    accent: read(`--color-forest-${name}-accent`),
  });
  return {
    seasons: { winter: season("winter"), spring: season("spring"), summer: season("summer"), autumn: season("autumn") },
    voidTile: read("--color-forest-void"),
    shadow: read("--color-forest-shadow"),
    outline: read("--color-forest-outline"),
    outlineToday: read("--color-forest-outline-today"),
    outlineHover: read("--color-forest-outline-hover"),
    label: read("--color-forest-label"),
    labelHalo: read("--color-forest-label-halo"),
    background: read("--color-surface-paper"),
    frontAlpha: number("--opacity-forest-front"),
    selectionFill: number("--opacity-forest-selection-fill"),
    font: `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`,
  };
}
