import type { FirstWeekday } from "./generated/FirstWeekday";
import type { ForestDay } from "./generated/ForestDay";
import type { Season } from "./generated/Season";

// A simplified copy of the Rust rules (`kinige_domain::forest` and `forest_year`) for `MockApi`, tests and stories only. The
// application never uses it: the real table comes from Rust. `forestLayout.test.ts` checks it against the table that Rust
// generated (`fixtures/*.json`), so the two cannot drift apart unnoticed.

const DAY_MS = 86_400_000;

/** Number of columns of the board and the cell of a date. */
export function mockLayout(year: number, first: FirstWeekday): { cols: number; cellOf: (date: Date) => { row: number; col: number } } {
  const jan1 = new Date(Date.UTC(year, 0, 1));
  const offset = first === "monday" ? (jan1.getUTCDay() + 6) % 7 : jan1.getUTCDay();
  const days = Math.round((Date.UTC(year + 1, 0, 1) - jan1.getTime()) / DAY_MS);
  return {
    cols: Math.ceil((offset + days) / 7),
    cellOf: (date) => {
      const index = offset + Math.round((date.getTime() - jan1.getTime()) / DAY_MS);
      return { row: index % 7, col: Math.floor(index / 7) };
    },
  };
}

export function mockSeason(month: number): Season {
  if (month >= 3 && month <= 5) return "spring";
  if (month >= 6 && month <= 8) return "summer";
  if (month >= 9 && month <= 11) return "autumn";
  return "winter";
}

/** `w = Σ min(1, len/200)`, short notes (under 3 characters) do not count; `stage = clamp(round(6·ln(1+w)/ln(31)), 0, 6)`. */
export function mockStage(lengths: readonly number[]): number {
  const weight = lengths.filter((len) => len >= 3).reduce((sum, len) => sum + Math.min(1, len / 200), 0);
  if (weight <= 0) return 0;
  return Math.min(6, Math.max(0, Math.round((6 * Math.log1p(weight)) / Math.log1p(30))));
}

export interface MockDayFacts {
  lengths: number[];
  excerpt: string | null;
  /** Live photos of the day (slice 7). */
  photos?: number;
}

/** Every day of `year` in calendar order with the given per-day facts (keyed by `YYYY-MM-DD`). */
export function mockDays(year: number, first: FirstWeekday, facts: ReadonlyMap<string, MockDayFacts>): { cols: number; days: ForestDay[] } {
  const { cols, cellOf } = mockLayout(year, first);
  const days: ForestDay[] = [];
  for (let date = new Date(Date.UTC(year, 0, 1)); date.getUTCFullYear() === year; date = new Date(date.getTime() + DAY_MS)) {
    const key = date.toISOString().slice(0, 10);
    const day = facts.get(key);
    days.push({
      date: key,
      ...cellOf(date),
      season: mockSeason(date.getUTCMonth() + 1),
      notes: day?.lengths.length ?? 0,
      stage: mockStage(day?.lengths ?? []),
      excerpt: day?.excerpt ?? null,
      photos: day?.photos ?? 0,
      firstSummary: null,
    });
  }
  return { cols, days };
}
