// Пары «цвет / фон», которые обязаны проходить проверку в каждой палитре.
// Порог выбирается по роли пары. Текст проверяется строже всего (4,5:1), поэтому крупный текст (порог 3:1) проходит заведомо.

export type PairKind = "text" | "ui" | "decorative";

/** Минимальный контраст по виду пары. decorative — порог проекта, не WCAG: граница или заливка заметна, но тонка. */
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
  "surface.field-active",
  "highlight.background",
];
const TEXT_ROLES = ["text.primary", "text.secondary", "text.tertiary", "accent.default", "status.error"];

/**
 * Цели пользователя по живому окну (ADR-0004): на плашке основной текст ≥ 12:1, вторичный ≥ 9:1,
 * время, ошибка и акцент ≥ 5:1. Выше порога WCAG, поэтому проверяются отдельным тестом.
 */
export const MESSAGE_TARGETS: { foreground: string; min: number }[] = [
  { foreground: "text.primary", min: 12 },
  { foreground: "text.secondary", min: 9 },
  { foreground: "text.tertiary", min: 5 },
  { foreground: "status.error", min: 5 },
  { foreground: "accent.default", min: 5 },
];

export const CONTRAST_PAIRS: ContrastPair[] = [
  ...TEXT_ROLES.flatMap((foreground) =>
    SURFACES.map((background) => ({ foreground, background, kind: "text" as const })),
  ),
  ...SURFACES.map((background) => ({ foreground: "focus.ring", background, kind: "ui" as const })),
  // Разделитель между сайдбаром и лентой (граница сайдбара) и черта пустого чекбокса идут по роли разделителя / третичного текста.
  { foreground: "border.divider", background: "surface.background", kind: "decorative" },
  { foreground: "border.divider", background: "surface.paper", kind: "decorative" },
  // Заливки без рамки отличимы от соседей по порогу проекта: плашка и поле от ленты, поле и выбранный чат от сайдбара,
  // подсвеченная плашка от обычной, блок кода от плашки.
  { foreground: "surface.message", background: "surface.paper", kind: "decorative" },
  { foreground: "surface.day", background: "surface.paper", kind: "decorative" },
  { foreground: "surface.field", background: "surface.paper", kind: "decorative" },
  { foreground: "surface.field", background: "surface.background", kind: "decorative" },
  { foreground: "surface.selected", background: "surface.background", kind: "decorative" },
  { foreground: "highlight.background", background: "surface.message", kind: "decorative" },
  // Выбранное без рамки, одной заливкой: вариант переключателя на поле (surface.field), выбранная копия и найденная строка
  // поиска на бумаге и на заливке наведения (surface.selected).
  { foreground: "highlight.background", background: "surface.field", kind: "decorative" },
  { foreground: "highlight.background", background: "surface.paper", kind: "decorative" },
  { foreground: "highlight.background", background: "surface.selected", kind: "decorative" },
  // Найденное слово (слайс 4): заливка заметна и на плашке, и на подсвеченной плашке; на ней стоит только основной и
  // вторичный текст (третичный и акцент на неё не попадают: слова в ссылках и тегах не подсвечиваются).
  { foreground: "highlight.mark", background: "surface.message", kind: "decorative" },
  { foreground: "highlight.mark", background: "highlight.background", kind: "decorative" },
  { foreground: "text.primary", background: "highlight.mark", kind: "text" },
  { foreground: "text.secondary", background: "highlight.mark", kind: "text" },
  { foreground: "surface.message", background: "surface.code", kind: "decorative" },
  // Пустой чекбокс задачи (граница компонента, WCAG 1.4.11) нарисован цветом третичного текста на плашке.
  { foreground: "text.tertiary", background: "surface.message", kind: "ui" },
];
