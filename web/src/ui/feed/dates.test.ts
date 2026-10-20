import { afterEach, describe, expect, it } from "vitest";
import { continuesSeries, formatDayLabel, formatTime, isSameDay } from "./dates";

// Даты строятся конструктором в местном времени, как и в самой функции: тест не зависит от часового пояса машины.
const at = (year: number, month: number, day: number, hour = 12, minute = 0) => new Date(year, month - 1, day, hour, minute);

describe("formatTime", () => {
  it("24 часа, две цифры", () => {
    expect(formatTime(at(2026, 10, 20, 9, 12))).toBe("09:12");
    expect(formatTime(at(2026, 10, 20, 0, 5))).toBe("00:05");
    expect(formatTime(at(2026, 10, 20, 12, 0))).toBe("12:00");
    expect(formatTime(at(2026, 10, 20, 23, 59))).toBe("23:59");
  });
});

describe("formatDayLabel", () => {
  const now = at(2026, 10, 20, 17, 30);

  it("сегодня и вчера", () => {
    expect(formatDayLabel(at(2026, 10, 20, 0, 0), now)).toBe("Сегодня");
    expect(formatDayLabel(at(2026, 10, 20, 23, 59), now)).toBe("Сегодня");
    expect(formatDayLabel(at(2026, 10, 19, 21, 5), now)).toBe("Вчера");
  });

  it("вчера через полночь: сообщение в 23:58, сейчас 00:03 следующего дня", () => {
    const justAfterMidnight = at(2026, 10, 21, 0, 3);
    expect(formatDayLabel(at(2026, 10, 20, 23, 58), justAfterMidnight)).toBe("Вчера");
    expect(formatDayLabel(at(2026, 10, 21, 0, 1), justAfterMidnight)).toBe("Сегодня");
    expect(formatDayLabel(at(2026, 10, 19, 23, 58), justAfterMidnight)).toBe("19 октября");
  });

  it("текущий год: день и месяц в родительном падеже, без года и без дня недели", () => {
    expect(formatDayLabel(at(2026, 3, 14), now)).toBe("14 марта");
    expect(formatDayLabel(at(2026, 1, 1), now)).toBe("1 января");
    expect(formatDayLabel(at(2026, 10, 18), now)).toBe("18 октября");
  });

  it("прошлые годы: с годом", () => {
    expect(formatDayLabel(at(2025, 3, 14), now)).toBe("14 марта 2025");
    expect(formatDayLabel(at(2024, 12, 31), now)).toBe("31 декабря 2024");
  });

  it("граница года: 31 декабря «вчера» после полуночи, на день раньше уже с годом", () => {
    const newYear = at(2027, 1, 1, 0, 5);
    expect(formatDayLabel(at(2026, 12, 31, 23, 59), newYear)).toBe("Вчера");
    expect(formatDayLabel(at(2026, 12, 30, 12, 0), newYear)).toBe("30 декабря 2026");
    expect(formatDayLabel(at(2027, 1, 1, 0, 1), newYear)).toBe("Сегодня");
  });

  it("все двенадцать месяцев в родительном падеже", () => {
    const names = [
      "января",
      "февраля",
      "марта",
      "апреля",
      "мая",
      "июня",
      "июля",
      "августа",
      "сентября",
      "октября",
      "ноября",
      "декабря",
    ];
    const lateNow = at(2026, 12, 31, 23, 0);
    names.forEach((name, index) => {
      // 5-е число каждого месяца: ни «Сегодня», ни «Вчера».
      expect(formatDayLabel(at(2026, index + 1, 5), lateNow)).toBe(`5 ${name}`);
    });
  });

  it("високосный день", () => {
    expect(formatDayLabel(at(2024, 2, 29), now)).toBe("29 февраля 2024");
  });

  it("месяц назад и далее считается по календарным дням, а не по часам: 24 ч назад утром — вчера", () => {
    expect(formatDayLabel(at(2026, 10, 19, 6, 0), at(2026, 10, 20, 5, 0))).toBe("Вчера");
    expect(formatDayLabel(at(2026, 10, 19, 6, 0), at(2026, 10, 19, 23, 0))).toBe("Сегодня");
  });
});

describe("переход на летнее время", () => {
  const original = process.env.TZ;
  afterEach(() => {
    if (original === undefined) delete process.env.TZ;
    else process.env.TZ = original;
  });

  it("в сутки переключения часов «вчера» остаётся вчера (America/New_York, 8 марта 2026: 23 часа)", () => {
    process.env.TZ = "America/New_York";
    // Смена пояса действительно применилась: после перехода на летнее время смещение 240 минут, до него — 300.
    expect(at(2026, 3, 8, 12).getTimezoneOffset()).toBe(240);
    expect(at(2026, 3, 7, 12).getTimezoneOffset()).toBe(300);
    expect(formatDayLabel(at(2026, 3, 8, 23, 30), at(2026, 3, 9, 0, 30))).toBe("Вчера");
    expect(formatDayLabel(at(2026, 3, 7, 23, 30), at(2026, 3, 9, 0, 30))).toBe("7 марта");
  });

  it("осенью, в сутки на 25 часов (1 ноября 2026), тоже", () => {
    process.env.TZ = "America/New_York";
    expect(formatDayLabel(at(2026, 11, 1, 0, 30), at(2026, 11, 2, 23, 30))).toBe("Вчера");
    expect(formatDayLabel(at(2026, 10, 31, 12, 0), at(2026, 11, 2, 0, 30))).toBe("31 октября");
  });
});

describe("isSameDay и continuesSeries", () => {
  it("тот же календарный день", () => {
    expect(isSameDay(at(2026, 10, 20, 0, 0), at(2026, 10, 20, 23, 59))).toBe(true);
    expect(isSameDay(at(2026, 10, 20, 23, 59), at(2026, 10, 21, 0, 0))).toBe(false);
  });

  it("серия: пауза меньше 5 минут", () => {
    expect(continuesSeries(at(2026, 10, 20, 9, 12), at(2026, 10, 20, 9, 14))).toBe(true);
    expect(continuesSeries(at(2026, 10, 20, 9, 0), at(2026, 10, 20, 9, 4))).toBe(true);
  });

  it("ровно 5 минут и больше — уже новая серия", () => {
    expect(continuesSeries(at(2026, 10, 20, 9, 0), at(2026, 10, 20, 9, 5))).toBe(false);
    expect(continuesSeries(at(2026, 10, 20, 9, 0), at(2026, 10, 20, 11, 30))).toBe(false);
  });

  it("через полночь серия не продолжается, даже если пауза мала", () => {
    expect(continuesSeries(at(2026, 10, 20, 23, 58), at(2026, 10, 21, 0, 1))).toBe(false);
  });

  it("сообщения не по порядку не образуют серию", () => {
    expect(continuesSeries(at(2026, 10, 20, 9, 14), at(2026, 10, 20, 9, 12))).toBe(false);
  });
});
