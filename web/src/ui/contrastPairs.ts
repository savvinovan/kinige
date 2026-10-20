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

// Любой текст может оказаться на любой заливке: проверяем все сочетания, а не только «задуманные».
const SURFACES = [
  "surface.background",
  "surface.paper",
  "surface.message",
  "surface.code",
  "surface.day",
  "surface.selected",
  "surface.field",
];
const TEXT_ROLES = ["text.primary", "text.secondary", "text.tertiary", "accent.default", "status.error"];

export const CONTRAST_PAIRS: ContrastPair[] = [
  ...TEXT_ROLES.flatMap((foreground) =>
    SURFACES.map((background) => ({ foreground, background, kind: "text" as const })),
  ),
  ...SURFACES.map((background) => ({ foreground: "focus.ring", background, kind: "ui" as const })),
  // Рамка подсвеченного сообщения лежит на плашке и на бумаге вокруг неё.
  { foreground: "highlight.border", background: "surface.message", kind: "ui" },
  { foreground: "highlight.border", background: "surface.paper", kind: "ui" },
  // Рамка элемента управления — граница компонента, WCAG 1.4.11: 3:1 к соседним цветам.
  // Фон и бумага — поля сайдбара и composer; плашка — пустой чекбокс задачи; заливка поля — внутренняя сторона рамки.
  { foreground: "border.control", background: "surface.background", kind: "ui" },
  { foreground: "border.control", background: "surface.paper", kind: "ui" },
  { foreground: "border.control", background: "surface.message", kind: "ui" },
  { foreground: "border.control", background: "surface.field", kind: "ui" },
  { foreground: "border.divider", background: "surface.background", kind: "decorative" },
  { foreground: "border.divider", background: "surface.paper", kind: "decorative" },
  // Черта цитаты стоит на плашке.
  { foreground: "border.divider", background: "surface.message", kind: "decorative" },
  // Плашка сообщения и метка дня отличимы от ленты, выбранный чат — от сайдбара (заливка без рамки, поэтому порог проекта).
  { foreground: "surface.message", background: "surface.paper", kind: "decorative" },
  { foreground: "surface.day", background: "surface.paper", kind: "decorative" },
  { foreground: "surface.selected", background: "surface.background", kind: "decorative" },
];
