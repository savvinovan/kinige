import type { FirstWeekday } from "../../api/generated/FirstWeekday";
import type { WeekdaySource } from "../../api/generated/WeekdaySource";

// The first weekday of the board follows the system locale (ADR-0013): `Intl.Locale.prototype.getWeekInfo()`, then the older
// `weekInfo` property, then Monday. Real engines differ (WKWebView, WebView2, WebKitGTK), which is why the window reports which
// of the three answered (debug log line, once per session). The board has two layouts, Monday-first and Sunday-first; a locale
// that starts its week on another day gets Monday.

export interface FirstWeekdayChoice {
  value: FirstWeekday;
  source: WeekdaySource;
}

/** The ECMAScript `weekInfo` shape; both members are optional in older engines. */
interface WeekInfoLocale {
  getWeekInfo?: () => { firstDay?: unknown };
  weekInfo?: { firstDay?: unknown };
}

/** `firstDay` is 1..7 with 1 = Monday and 7 = Sunday; only those two are boards, anything else is "not an answer". */
function boardDay(firstDay: unknown): FirstWeekday | null {
  if (firstDay === 1) return "monday";
  if (firstDay === 7) return "sunday";
  return null;
}

export function detectFirstWeekday(language: string = typeof navigator === "undefined" ? "" : navigator.language): FirstWeekdayChoice {
  let locale: WeekInfoLocale | null = null;
  try {
    locale = new Intl.Locale(language) as unknown as WeekInfoLocale;
  } catch {
    // An unknown or malformed language tag: the fallback below.
  }
  if (locale) {
    try {
      const value = typeof locale.getWeekInfo === "function" ? boardDay(locale.getWeekInfo().firstDay) : null;
      if (value) return { value, source: "get_week_info" };
    } catch {
      // The method exists but failed: try the property.
    }
    try {
      const value = boardDay(locale.weekInfo?.firstDay);
      if (value) return { value, source: "week_info" };
    } catch {
      // A getter that throws: the fallback.
    }
  }
  return { value: "monday", source: "fallback" };
}
