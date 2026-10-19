// Пары «цвет / фон», которые обязаны проходить проверку в каждой палитре.
// Порог выбирается по роли пары. Текст проверяется строже всего (4,5:1), поэтому крупный текст (порог 3:1) проходит заведомо.

export type PairKind = "text" | "ui" | "decorative";

/** Минимальный контраст по виду пары. decorative — порог проекта, не WCAG: разделитель виден, но тонок. */
export const MIN_RATIO: Record<PairKind, number> = { text: 4.5, ui: 3, decorative: 1.2 };

export interface ContrastPair {
  foreground: string;
  background: string;
  kind: PairKind;
}

const SURFACES = ["surface.background", "surface.paper", "highlight.background"];
const TEXT_ROLES = ["text.primary", "text.secondary", "text.tertiary", "accent.default", "status.error"];

export const CONTRAST_PAIRS: ContrastPair[] = [
  ...TEXT_ROLES.flatMap((foreground) =>
    SURFACES.map((background) => ({ foreground, background, kind: "text" as const })),
  ),
  ...SURFACES.map((background) => ({ foreground: "focus.ring", background, kind: "ui" as const })),
  { foreground: "border.divider", background: "surface.background", kind: "decorative" },
  { foreground: "border.divider", background: "surface.paper", kind: "decorative" },
];
