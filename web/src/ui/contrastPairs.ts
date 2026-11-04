// "Color / background" pairs that must pass the check in every palette.
// The threshold is chosen by the pair's kind. Text is checked the strictest (4.5:1), so large text (threshold 3:1) passes for sure.
import { TILE_ROLES } from "./tokenSource";

export type PairKind = "text" | "ui" | "selection" | "decorative" | "hover";

/**
 * Minimum contrast by pair kind. decorative is a project threshold, not WCAG: a border or fill is noticeable but thin. selection is also a
 * project threshold: the selected item is shown by a fill alone, without a border, and it is the only sign of selection (the search row under the arrows). hover is
 * the highlight under the mouse, the quietest of all fills: it is temporary, and the selected row next to it stays more noticeable (decorative, 1.2:1).
 */
export const MIN_RATIO: Record<PairKind, number> = { text: 4.5, ui: 3, selection: 1.5, decorative: 1.2, hover: 1.15 };

export interface ContrastPair {
  foreground: string;
  background: string;
  kind: PairKind;
}

// Any text can end up on any fill: we check all combinations, not only the "intended" ones.
const SURFACES = [
  "surface.background",
  "surface.paper",
  "surface.message",
  "surface.code",
  "surface.day",
  "surface.selected",
  "surface.hover",
  "surface.card",
  "surface.field",
  "surface.field-active",
  "highlight.background",
];
const TEXT_ROLES = ["text.primary", "text.secondary", "text.tertiary", "accent.default", "status.error"];

/**
 * The user's targets from the live window (ADR-0004): on the message bubble, primary text >= 12:1, secondary >= 9:1,
 * time, error and accent >= 5:1. Above the WCAG threshold, so they are checked by a separate test.
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
  // The dot of the recording bar (slice 7a) is a shape, not text: the UI threshold on every surface it can sit on.
  ...SURFACES.map((background) => ({ foreground: "status.recording", background, kind: "ui" as const })),
  // The unplayed bars of a voice note's waveform (slice 7a) are shapes on the message plate: the UI threshold there. On the highlighted plate
  // (a short-lived jump target) they only have to stay visible, and they are a step away from the played bars (accent) on the plate.
  { foreground: "voice.unplayed", background: "surface.message", kind: "ui" },
  { foreground: "voice.unplayed", background: "highlight.background", kind: "decorative" },
  // The divider between the sidebar and the feed (the sidebar border) and the empty checkbox outline use the divider / tertiary text role.
  { foreground: "border.divider", background: "surface.background", kind: "decorative" },
  { foreground: "border.divider", background: "surface.paper", kind: "decorative" },
  // Fills without a border are distinguishable from their neighbors by the project threshold: the bubble and the field from the feed, the field and the selected chat from the sidebar,
  // the highlighted bubble from the regular one, the code block from the bubble.
  { foreground: "surface.message", background: "surface.paper", kind: "decorative" },
  { foreground: "surface.day", background: "surface.paper", kind: "decorative" },
  { foreground: "surface.card", background: "surface.paper", kind: "decorative" },
  { foreground: "surface.field", background: "surface.paper", kind: "decorative" },
  { foreground: "surface.field", background: "surface.background", kind: "decorative" },
  { foreground: "surface.selected", background: "surface.background", kind: "decorative" },
  { foreground: "highlight.background", background: "surface.message", kind: "decorative" },
  // Tile with a letter (D2): a white letter on the tile reads as regular text (>= 4.5:1) on every tile color.
  ...TILE_ROLES.map((tile) => ({ foreground: "text.primary", background: tile, kind: "text" as const })),
  // Sidebar (D2): the hover highlight is visible against the window background; the chat tile is visible on the background, under the mouse and on the selected row.
  { foreground: "surface.hover", background: "surface.background", kind: "hover" },
  // The selected row is clearly different from the row under the mouse (and from the background: the selected / background pair above).
  { foreground: "surface.selected", background: "surface.hover", kind: "decorative" },
  ...TILE_ROLES.flatMap((tile) =>
    ["surface.background", "surface.hover", "surface.selected"].map((background) => ({ foreground: tile, background, kind: "decorative" as const })),
  ),
  // Selected without a border, by a fill alone: a switch option on the field (surface.field), the selected copy and the found search
  // row on paper and on the hover fill (surface.selected).
  { foreground: "highlight.background", background: "surface.field", kind: "decorative" },
  { foreground: "highlight.background", background: "surface.paper", kind: "decorative" },
  { foreground: "highlight.background", background: "surface.selected", kind: "decorative" },
  // The search row under the arrows: focus is in the field, the fill is the only sign. Clearly stronger than a regular bubble (1.5:1); on it
  // primary and secondary text are readable (tertiary is replaced by secondary in the selected row), and so is the found word.
  { foreground: "highlight.row", background: "surface.message", kind: "selection" },
  { foreground: "highlight.row", background: "surface.paper", kind: "selection" },
  { foreground: "text.primary", background: "highlight.row", kind: "text" },
  { foreground: "text.secondary", background: "highlight.row", kind: "text" },
  { foreground: "highlight.mark", background: "highlight.row", kind: "decorative" },
  // The found word (slice 4): the fill is visible on the bubble and on the highlighted bubble; only primary and
  // secondary text sit on it (tertiary and accent never land on it: words in links and tags are not highlighted).
  { foreground: "highlight.mark", background: "surface.message", kind: "decorative" },
  { foreground: "highlight.mark", background: "highlight.background", kind: "decorative" },
  { foreground: "text.primary", background: "highlight.mark", kind: "text" },
  { foreground: "text.secondary", background: "highlight.mark", kind: "text" },
  { foreground: "surface.message", background: "surface.code", kind: "decorative" },
  // The empty task checkbox (the component border, WCAG 1.4.11) is drawn in the tertiary text color on the bubble.
  { foreground: "text.tertiary", background: "surface.message", kind: "ui" },
];
