import { describe, expect, it } from "vitest";
import {
  addDays,
  addMonths,
  daysInMonth,
  firstWeekday,
  formatDate,
  formatLongDate,
  formatMonth,
  monthGrid,
  parseDate,
  weekBounds,
  weekday,
  weekdayName,
} from "./dateMath";

const ymd = (value: string) => {
  const date = parseDate(value);
  if (!date) throw new Error(`не дата: ${value}`);
  return date;
};

describe("parseDate / formatDate", () => {
  it("разбирает и собирает обратно", () => {
    expect(formatDate(ymd("2026-10-20"))).toBe("2026-10-20");
    expect(ymd("2026-01-05")).toEqual({ year: 2026, month: 1, day: 5 });
  });

  it("несуществующие и кривые даты — null", () => {
    for (const bad of ["2026-02-30", "2026-13-01", "2026-00-10", "2026-10-00", "26-10-20", "2026-1-5", "", "вчера", "2026-10-20T10:00"]) {
      expect(parseDate(bad), bad).toBeNull();
    }
    expect(parseDate("2024-02-29")).not.toBeNull();
    expect(parseDate("2026-02-29")).toBeNull();
  });
});

describe("сдвиги", () => {
  it("дни через границы месяца, года и високосный февраль", () => {
    expect(formatDate(addDays(ymd("2026-10-31"), 1))).toBe("2026-11-01");
    expect(formatDate(addDays(ymd("2026-12-31"), 1))).toBe("2027-01-01");
    expect(formatDate(addDays(ymd("2027-01-01"), -1))).toBe("2026-12-31");
    expect(formatDate(addDays(ymd("2024-02-28"), 1))).toBe("2024-02-29");
    expect(formatDate(addDays(ymd("2026-10-20"), 7))).toBe("2026-10-27");
  });

  it("месяцы ужимают день до длины месяца, год переходит", () => {
    expect(formatDate(addMonths(ymd("2026-01-31"), 1))).toBe("2026-02-28");
    expect(formatDate(addMonths(ymd("2024-01-31"), 1))).toBe("2024-02-29");
    expect(formatDate(addMonths(ymd("2026-12-15"), 1))).toBe("2027-01-15");
    expect(formatDate(addMonths(ymd("2026-01-15"), -1))).toBe("2025-12-15");
    expect(formatDate(addMonths(ymd("2026-03-31"), -1))).toBe("2026-02-28");
    expect(formatDate(addMonths(ymd("2026-10-20"), 14))).toBe("2027-12-20");
  });

  it("длина месяца и день недели", () => {
    expect(daysInMonth(2026, 2)).toBe(28);
    expect(daysInMonth(2024, 2)).toBe(29);
    expect(daysInMonth(2026, 10)).toBe(31);
    expect(weekday(ymd("2026-10-20"))).toBe(2); // вторник
    expect(weekday(ymd("2026-10-18"))).toBe(0); // воскресенье
  });
});

describe("первый день недели", () => {
  it("русский — понедельник, английский — воскресенье", () => {
    expect(firstWeekday("ru")).toBe(1);
    expect(firstWeekday("en")).toBe(0);
  });
});

describe("monthGrid", () => {
  it("октябрь 2026 с понедельника: пять недель, первая начинается 28 сентября", () => {
    const weeks = monthGrid(2026, 10, 1);
    expect(weeks).toHaveLength(5);
    expect(weeks.every((week) => week.length === 7)).toBe(true);
    expect(formatDate(weeks[0]?.[0]?.date ?? ymd("2000-01-01"))).toBe("2026-09-28");
    expect(weeks[0]?.slice(0, 3).map((cell) => cell.inMonth)).toEqual([false, false, false]);
    expect(formatDate(weeks[4]?.[6]?.date ?? ymd("2000-01-01"))).toBe("2026-11-01");
    expect(weeks.flat().filter((cell) => cell.inMonth)).toHaveLength(31);
  });

  it("с воскресенья та же сетка сдвигается", () => {
    const weeks = monthGrid(2026, 10, 0);
    expect(formatDate(weeks[0]?.[0]?.date ?? ymd("2000-01-01"))).toBe("2026-09-27");
    expect(weeks.flat()).toHaveLength(35);
  });

  it("февраль 2027 ровно в четыре недели, если начинается с первого дня недели", () => {
    // 1 февраля 2027 — понедельник, 28 дней.
    expect(monthGrid(2027, 2, 1)).toHaveLength(4);
  });

  it("месяц, которому нужно шесть недель", () => {
    // Март 2025: 1-е — суббота, 31 день, неделя с понедельника → 6 строк.
    expect(monthGrid(2025, 3, 1)).toHaveLength(6);
  });
});

describe("границы недели", () => {
  it("с понедельника: вторник 20 → 19–25 октября; с воскресенья: 18–24", () => {
    const monday = weekBounds(ymd("2026-10-20"), 1);
    expect([formatDate(monday.start), formatDate(monday.end)]).toEqual(["2026-10-19", "2026-10-25"]);
    const sunday = weekBounds(ymd("2026-10-20"), 0);
    expect([formatDate(sunday.start), formatDate(sunday.end)]).toEqual(["2026-10-18", "2026-10-24"]);
  });

  it("воскресенье при неделе с понедельника — её конец", () => {
    const week = weekBounds(ymd("2026-10-25"), 1);
    expect([formatDate(week.start), formatDate(week.end)]).toEqual(["2026-10-19", "2026-10-25"]);
  });
});

describe("подписи", () => {
  it("месяц: «октябрь 2026» без « г.», «October 2026»", () => {
    expect(formatMonth(2026, 10, "ru")).toBe("октябрь 2026");
    expect(formatMonth(2026, 10, "en")).toBe("October 2026");
  });

  it("полная дата и дни недели", () => {
    expect(formatLongDate(ymd("2026-10-20"), "en")).toBe("Tuesday, October 20, 2026");
    expect(formatLongDate(ymd("2026-10-20"), "ru")).toContain("вторник");
    expect(weekdayName(1, "en", "short")).toBe("Mon");
    expect(weekdayName(1, "ru", "long")).toBe("понедельник");
    expect(weekdayName(0, "en", "long")).toBe("Sunday");
  });
});
