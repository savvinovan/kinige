import { mockDays, type MockDayFacts } from "../../api/mockForest";
import type { FirstWeekday } from "../../api/generated/FirstWeekday";
import type { ForestYear } from "../../api/generated/ForestYear";

/**
 * A year table built with the mock layout (`mockForest.ts`, checked against the Rust tables in `forestLayout.test.ts`) for tests
 * and stories that need a year the fixtures do not have: an empty one, another first weekday, other facts.
 */
export function demoYear(
  year: number,
  options: { first?: FirstWeekday; today?: string; minYear?: number; facts?: ReadonlyMap<string, MockDayFacts> } = {},
): ForestYear {
  const first = options.first ?? "monday";
  const { cols, days } = mockDays(year, first, options.facts ?? new Map());
  return { year, firstWeekday: first, cols, minYear: options.minYear ?? year, today: options.today ?? `${year}-12-31`, days };
}
