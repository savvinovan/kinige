import { describe, expect, it } from "vitest";
import { createI18n } from "../../i18n";
import {
  dayAnnouncement,
  daySummary,
  longDay,
  monthNames,
  notesText,
  photosText,
  shortDay,
  stageName,
  weekdayLetters,
} from "./forestText";

const en = createI18n("en").t;
const ru = createI18n("ru").t;

describe("words of the forest", () => {
  it("dates in both languages, without the Russian year suffix", () => {
    expect(longDay("2026-10-16", "en")).toBe("Fri, October 16, 2026");
    expect(longDay("2026-10-16", "ru")).toBe("пт, 16 октября 2026");
    expect(shortDay("2026-10-16", "en")).toBe("October 16");
    expect(shortDay("2026-10-16", "ru")).toBe("16 октября");
    expect(monthNames("en")).toHaveLength(12);
    expect(monthNames("en", "long")[9]).toBe("October");
    expect(monthNames("ru", "narrow")).toHaveLength(12);
  });

  it("notes: the English plural", () => {
    expect([0, 1, 2, 5, 21].map((n) => notesText(en, n))).toEqual(["0 notes", "1 note", "2 notes", "5 notes", "21 notes"]);
  });

  it("notes: the Russian plural forms one, few, many", () => {
    const text = (n: number) => notesText(ru, n).replace(/\s/g, " ");
    expect([1, 2, 4, 5, 11, 12, 21, 22, 25, 100, 101].map(text)).toEqual([
      "1 запись",
      "2 записи",
      "4 записи",
      "5 записей",
      "11 записей",
      "12 записей",
      "21 запись",
      "22 записи",
      "25 записей",
      "100 записей",
      "101 запись",
    ]);
  });

  it("seven stage names in each language; out-of-range stages are clamped", () => {
    const names = [0, 1, 2, 3, 4, 5, 6].map((stage) => stageName(en, stage));
    expect(names).toEqual(["bare ground", "a sprout", "a seedling", "a sapling", "a young tree", "a tree", "a large tree"]);
    expect(new Set([0, 1, 2, 3, 4, 5, 6].map((stage) => stageName(ru, stage))).size).toBe(7);
    expect(stageName(en, 99)).toBe("a large tree");
    expect(stageName(en, -4)).toBe("bare ground");
    expect(stageName(en, Number.NaN)).toBe("bare ground");
  });

  it("the card summary and what a screen reader says", () => {
    expect(daySummary(en, { notes: 5, stage: 6 })).toBe("5 notes · a large tree");
    expect(daySummary(en, { notes: 0, stage: 0 })).toBe("No notes");
    expect(dayAnnouncement(en, "en", { date: "2026-10-16", notes: 5, stage: 6 })).toBe("October 16, 5 notes, a large tree");
    expect(dayAnnouncement(en, "en", { date: "2026-10-17", notes: 0, stage: 0 })).toBe("October 17, No notes");
    expect(dayAnnouncement(ru, "ru", { date: "2026-10-16", notes: 5, stage: 6 })).toBe("16 октября, 5 записей, большое дерево");
  });

  it("the photos of a day (slice 7): \"5 notes · 3 photos · a large tree\", only when there are photos", () => {
    expect(daySummary(en, { notes: 5, stage: 6, photos: 3 })).toBe("5 notes · 3 photos · a large tree");
    expect(daySummary(en, { notes: 1, stage: 2, photos: 1 })).toBe("1 note · 1 photo · a seedling");
    expect(daySummary(en, { notes: 5, stage: 6, photos: 0 })).toBe("5 notes · a large tree");
    expect(daySummary(en, { notes: 0, stage: 0, photos: 4 })).toBe("No notes");
    expect(dayAnnouncement(en, "en", { date: "2026-10-16", notes: 5, stage: 6, photos: 3 })).toBe(
      "October 16, 5 notes, 3 photos, a large tree",
    );
    const ruPhotos = (photos: number) => photosText(ru, photos).replace(/\s/g, " ");
    // The card is short: "фото" does not decline.
    expect([1, 2, 5, 21].map(ruPhotos)).toEqual(["1 фото", "2 фото", "5 фото", "21 фото"]);
    expect(daySummary(ru, { notes: 5, stage: 6, photos: 3 }).replace(/\s/g, " ")).toBe("5 записей · 3 фото · большое дерево");
  });

  it("weekday letters follow the rows of the board", () => {
    const monday = weekdayLetters(["2026-10-05", "2026-10-06", null], "en");
    expect(monday).toEqual(["M", "T", ""]);
  });
});
