// Pure calendar logic of the forest: the position of a day in both candidate grids, seasons, the growth formulas of
// PRD §7.2. No DOM, no canvas. The Rust candidate lives in crates/kinige-domain/src/forest.rs and shares the golden
// table at the bottom of calendar.test.ts.

export type GridKind = "A" | "B";
export type FirstWeekday = "mon" | "sun";
export type Season = "winter" | "spring" | "summer" | "autumn";

export interface Cell {
  row: number;
  col: number;
}

export interface GridDims {
  rows: number;
  cols: number;
}

/** A calendar date without a time zone; `m` is 1..12. */
export interface YmD {
  y: number;
  m: number;
  d: number;
}

export function isLeap(y: number): boolean {
  return (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
}

export function daysInYear(y: number): number {
  return isLeap(y) ? 366 : 365;
}

export function daysInMonth(y: number, m: number): number {
  return new Date(Date.UTC(y, m, 0)).getUTCDate();
}

/** Weekday with Monday = 0 … Sunday = 6. */
export function weekdayMon0(y: number, m: number, d: number): number {
  return (new Date(Date.UTC(y, m - 1, d)).getUTCDay() + 6) % 7;
}

/** Day of the year, 0-based (1 January = 0). */
export function ordinal0(y: number, m: number, d: number): number {
  return Math.round((Date.UTC(y, m - 1, d) - Date.UTC(y, 0, 1)) / 86_400_000);
}

export function dateFromOrdinal0(y: number, ord: number): YmD {
  const t = new Date(Date.UTC(y, 0, 1 + ord));
  return { y: t.getUTCFullYear(), m: t.getUTCMonth() + 1, d: t.getUTCDate() };
}

export function isValidDate(y: number, m: number, d: number): boolean {
  return m >= 1 && m <= 12 && d >= 1 && d <= daysInMonth(y, m);
}

export function isoKey({ y, m, d }: YmD): string {
  return `${y}-${String(m).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
}

/** How far 1 January sits from the start of its week: the empty cells above it in the first column of grid B. */
export function januaryOffset(year: number, first: FirstWeekday): number {
  const firstIdx = first === "mon" ? 0 : 6;
  return (weekdayMon0(year, 1, 1) - firstIdx + 7) % 7;
}

/**
 * Size of the grid for a year. A is always 12 × 31. B is 7 rows and 53 columns, but a leap year whose 1 January is the
 * last day of the week (2012 and 2040 with a Monday start, 2028 and 2056 with a Sunday start) needs a 54th column:
 * 6 empty cells + 366 days = 372 > 7 × 53.
 */
export function gridDims(grid: GridKind, year: number, first: FirstWeekday): GridDims {
  if (grid === "A") return { rows: 12, cols: 31 };
  const cells = januaryOffset(year, first) + daysInYear(year);
  return { rows: 7, cols: Math.ceil(cells / 7) };
}

/** The position of a day: `(year, date, first_weekday) → (row, col)`; `null` for a date that does not exist. */
export function dayPosition(grid: GridKind, { y, m, d }: YmD, first: FirstWeekday): Cell | null {
  if (!isValidDate(y, m, d)) return null;
  if (grid === "A") return { row: m - 1, col: d - 1 };
  const idx = januaryOffset(y, first) + ordinal0(y, m, d);
  return { row: idx % 7, col: Math.floor(idx / 7) };
}

/** The inverse of `dayPosition`: the date shown in a cell, `null` for an empty ground cell. */
export function dateAtCell(grid: GridKind, year: number, first: FirstWeekday, { row, col }: Cell): YmD | null {
  const dims = gridDims(grid, year, first);
  if (row < 0 || col < 0 || row >= dims.rows || col >= dims.cols) return null;
  if (grid === "A") return isValidDate(year, row + 1, col + 1) ? { y: year, m: row + 1, d: col + 1 } : null;
  const ord = col * 7 + row - januaryOffset(year, first);
  if (ord < 0 || ord >= daysInYear(year)) return null;
  return dateFromOrdinal0(year, ord);
}

export function allDays(year: number): YmD[] {
  const out: YmD[] = [];
  for (let i = 0; i < daysInYear(year); i++) out.push(dateFromOrdinal0(year, i));
  return out;
}

/** Season of a month (PRD §7.3); `southern` shifts it by six months. */
export function seasonOfMonth(m: number, southern = false): Season {
  const mm = southern ? ((m + 5) % 12) + 1 : m;
  if (mm === 12 || mm <= 2) return "winter";
  if (mm <= 5) return "spring";
  if (mm <= 8) return "summer";
  return "autumn";
}

// ---- growth (PRD §7.2) -------------------------------------------------------------------------------------------

export const W_MAX = 30;
export const MAX_STAGE = 6;

export interface NoteStat {
  /** Length in characters. */
  len: number;
  attachments: number;
}

/** `w = Σ min(1, len/200) + 0.5 × attachments`; a message shorter than 3 characters without attachments does not count. */
export function dayWeight(notes: NoteStat[]): number {
  let w = 0;
  for (const n of notes) {
    if (n.len < 3 && n.attachments === 0) continue;
    w += Math.min(1, n.len / 200) + 0.5 * n.attachments;
  }
  return w;
}

/** `clamp(round(6 · ln(1+w) / ln(1+w_max)), 0, 6)`. */
export function stageOf(w: number, wMax = W_MAX): number {
  const s = Math.round((MAX_STAGE * Math.log(1 + w)) / Math.log(1 + wMax));
  return Math.min(MAX_STAGE, Math.max(0, s));
}

// ---- synthetic data ----------------------------------------------------------------------------------------------

export interface DayData {
  notes: number;
  photos: number;
  weight: number;
  stage: number;
  firstLine: string;
}

const LINES = [
  "Synthetic note: morning walk and a long list of small things",
  "Synthetic note: a draft of the plan for the week",
  "Synthetic note: read a few pages, wrote down two quotes",
  "Synthetic note: the call went better than expected",
  "Synthetic note: groceries, laundry, an idea for the weekend",
  "Synthetic note: nothing special, but a quiet day",
];

/** Deterministic PRNG (mulberry32), so every run and every browser draws the same forest. */
export function prng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * A full synthetic year. A latent "mood" drifts from day to day (a busy stretch, a quiet one), a few days and one
 * week-long gap are empty, the rest is a log-normal number of notes with log-normal lengths and an occasional photo.
 */
export function syntheticYear(year: number, seed = 20261102): Map<string, DayData> {
  const rnd = prng(seed);
  const normal = () => Math.sqrt(-2 * Math.log(1 - rnd())) * Math.cos(2 * Math.PI * rnd());
  const out = new Map<string, DayData>();
  let mood = 0;
  const gapStart = 205; // a week-long break in summer
  for (let i = 0; i < daysInYear(year); i++) {
    const date = dateFromOrdinal0(year, i);
    mood = 0.8 * mood + 0.6 * normal();
    const inGap = i >= gapStart && i < gapStart + 8;
    const skipped = rnd() < 0.07;
    let notes = 0;
    if (!inGap && !skipped) notes = Math.max(1, Math.round(Math.exp(2.3 + 0.8 * mood + 0.5 * normal())));
    const stats: NoteStat[] = [];
    let photos = 0;
    for (let k = 0; k < notes; k++) {
      const len = Math.max(1, Math.round(Math.exp(4.2 + 1.1 * normal())));
      const att = rnd() < 0.06 ? 1 : 0;
      photos += att;
      stats.push({ len, attachments: att });
    }
    const weight = dayWeight(stats);
    const line = LINES[Math.floor(rnd() * LINES.length)] ?? LINES[0] ?? "";
    out.set(isoKey(date), { notes, photos, weight, stage: stageOf(weight), firstLine: notes > 0 ? line : "" });
  }
  return out;
}

export function stageHistogram(data: Map<string, DayData>): number[] {
  const h = new Array<number>(MAX_STAGE + 1).fill(0);
  for (const v of data.values()) h[v.stage] = (h[v.stage] ?? 0) + 1;
  return h;
}
