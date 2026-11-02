import { afterEach, describe, expect, it, vi } from "vitest";
import { detectFirstWeekday } from "./firstWeekday";

// The fallback chain of ADR-0013 with a stubbed `Intl.Locale`: the real engines (WKWebView, WebView2) are probed by the runner
// scenarios, which read the debug line of the app log.
function stubLocale(make: () => object) {
  class FakeLocale {
    constructor() {
      Object.assign(this, make());
    }
  }
  vi.stubGlobal("Intl", { ...Intl, Locale: FakeLocale });
}

afterEach(() => vi.unstubAllGlobals());

describe("first weekday of the board", () => {
  it("getWeekInfo(): 1 is Monday and 7 is Sunday", () => {
    stubLocale(() => ({ getWeekInfo: () => ({ firstDay: 1, minimalDays: 4 }) }));
    expect(detectFirstWeekday("ru-RU")).toEqual({ value: "monday", source: "get_week_info" });
    stubLocale(() => ({ getWeekInfo: () => ({ firstDay: 7 }) }));
    expect(detectFirstWeekday("en-US")).toEqual({ value: "sunday", source: "get_week_info" });
  });

  it("only the older weekInfo property", () => {
    stubLocale(() => ({ weekInfo: { firstDay: 7 } }));
    expect(detectFirstWeekday("en-US")).toEqual({ value: "sunday", source: "week_info" });
    stubLocale(() => ({ weekInfo: { firstDay: 1 } }));
    expect(detectFirstWeekday("de")).toEqual({ value: "monday", source: "week_info" });
  });

  it("neither: Monday", () => {
    stubLocale(() => ({}));
    expect(detectFirstWeekday("en-US")).toEqual({ value: "monday", source: "fallback" });
  });

  it("getWeekInfo throws: the property is tried, then Monday", () => {
    stubLocale(() => ({
      getWeekInfo: () => {
        throw new Error("nope");
      },
    }));
    expect(detectFirstWeekday("en-US")).toEqual({ value: "monday", source: "fallback" });
    stubLocale(() => ({
      getWeekInfo: () => {
        throw new Error("nope");
      },
      weekInfo: { firstDay: 7 },
    }));
    expect(detectFirstWeekday("en-US")).toEqual({ value: "sunday", source: "week_info" });
  });

  it("garbage in firstDay is not an answer: 0, 8, NaN, a string, null, an object, Saturday (6)", () => {
    for (const firstDay of [0, 8, Number.NaN, "1", "7", null, undefined, {}, [1], 6, 2, -1, 1.5, Number.POSITIVE_INFINITY]) {
      stubLocale(() => ({ getWeekInfo: () => ({ firstDay }) }));
      expect(detectFirstWeekday("xx"), String(firstDay)).toEqual({ value: "monday", source: "fallback" });
    }
    stubLocale(() => ({ getWeekInfo: () => null, weekInfo: null }));
    expect(detectFirstWeekday("xx")).toEqual({ value: "monday", source: "fallback" });
    stubLocale(() => ({ getWeekInfo: "not a function", weekInfo: 5 }));
    expect(detectFirstWeekday("xx")).toEqual({ value: "monday", source: "fallback" });
  });

  it("a throwing weekInfo getter, a throwing constructor and a missing Intl.Locale are the fallback", () => {
    stubLocale(() => ({
      get weekInfo() {
        throw new Error("nope");
      },
    }));
    expect(detectFirstWeekday("en")).toEqual({ value: "monday", source: "fallback" });
    vi.stubGlobal("Intl", {
      ...Intl,
      Locale: class {
        constructor() {
          throw new RangeError("bad tag");
        }
      },
    });
    expect(detectFirstWeekday("x-bad tag!")).toEqual({ value: "monday", source: "fallback" });
    vi.stubGlobal("Intl", { ...Intl, Locale: undefined });
    expect(detectFirstWeekday("en")).toEqual({ value: "monday", source: "fallback" });
  });

  it("the real engine of the test run answers one of the three without throwing", () => {
    const real = detectFirstWeekday("en-US");
    expect(["monday", "sunday"]).toContain(real.value);
    expect(["get_week_info", "week_info", "fallback"]).toContain(real.source);
    expect(detectFirstWeekday("")).toBeDefined();
    expect(detectFirstWeekday("\u0000")).toEqual({ value: "monday", source: "fallback" });
  });
});
