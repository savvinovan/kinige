import { describe, expect, it } from "vitest";
import { SERIES_PAUSE_MS, continuesSeries, formatDayLabel, formatTime } from "./dates";

const RU = { today: "Сегодня", yesterday: "Вчера" };
const EN = { today: "Today", yesterday: "Yesterday" };

describe("formatTime", () => {
  // 2026-10-21 06:12 UTC = 09:12 в Москве, 23:12 накануне в Лос-Анджелесе.
  const moment = new Date(Date.UTC(2026, 9, 21, 6, 12));

  it("24 часа, две цифры, в поясе записи", () => {
    expect(formatTime(moment, "ru", "Europe/Moscow")).toBe("09:12");
    expect(formatTime(moment, "en", "Europe/Moscow")).toBe("09:12");
    expect(formatTime(moment, "ru", "America/Los_Angeles")).toBe("23:12");
    expect(formatTime(new Date(Date.UTC(2026, 9, 21, 0, 5)), "en", "UTC")).toBe("00:05");
    expect(formatTime(new Date(Date.UTC(2026, 9, 21, 12, 0)), "ru", "UTC")).toBe("12:00");
    expect(formatTime(new Date(Date.UTC(2026, 9, 21, 23, 59)), "en", "UTC")).toBe("23:59");
  });

  it("переход на летнее время: пояс записи, а не пояс окна", () => {
    // Берлин, 2026-03-29: 00:59 UTC = 01:59 (зима), 01:00 UTC = 03:00 (лето).
    expect(formatTime(new Date(Date.UTC(2026, 2, 29, 0, 59)), "ru", "Europe/Berlin")).toBe("01:59");
    expect(formatTime(new Date(Date.UTC(2026, 2, 29, 1, 0)), "ru", "Europe/Berlin")).toBe("03:00");
  });

  it("неизвестный пояс (повреждённые данные) не ломает ленту: время в поясе окна", () => {
    expect(() => formatTime(moment, "ru", "Не/Пояс")).not.toThrow();
    expect(formatTime(moment, "ru", "Не/Пояс")).toMatch(/^\d{2}:\d{2}$/);
  });
});

describe("formatDayLabel", () => {
  const today = "2026-10-20";

  it("сегодня и вчера — по journal_date", () => {
    expect(formatDayLabel("2026-10-20", today, "ru", RU)).toBe("Сегодня");
    expect(formatDayLabel("2026-10-19", today, "ru", RU)).toBe("Вчера");
    expect(formatDayLabel("2026-10-20", today, "en", EN)).toBe("Today");
    expect(formatDayLabel("2026-10-19", today, "en", EN)).toBe("Yesterday");
  });

  it("текущий год: день и месяц без года и без дня недели", () => {
    expect(formatDayLabel("2026-03-14", today, "ru", RU)).toBe("14 марта");
    expect(formatDayLabel("2026-01-01", today, "ru", RU)).toBe("1 января");
    expect(formatDayLabel("2026-10-18", today, "ru", RU)).toBe("18 октября");
    expect(formatDayLabel("2026-03-14", today, "en", EN)).toBe("March 14");
    expect(formatDayLabel("2026-10-18", today, "en", EN)).toBe("October 18");
  });

  it("прошлые годы: с годом, без « г.»", () => {
    expect(formatDayLabel("2025-03-14", today, "ru", RU)).toBe("14 марта 2025");
    expect(formatDayLabel("2024-12-31", today, "ru", RU)).toBe("31 декабря 2024");
    expect(formatDayLabel("2025-03-14", today, "en", EN)).toBe("March 14, 2025");
  });

  it("граница года: 31 декабря «вчера» 1 января, на день раньше уже с годом", () => {
    expect(formatDayLabel("2026-12-31", "2027-01-01", "ru", RU)).toBe("Вчера");
    expect(formatDayLabel("2026-12-30", "2027-01-01", "ru", RU)).toBe("30 декабря 2026");
    expect(formatDayLabel("2027-01-01", "2027-01-01", "ru", RU)).toBe("Сегодня");
  });

  it("все двенадцать месяцев в родительном падеже (ru) и по-английски", () => {
    const ru = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];
    const en = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    ru.forEach((name, index) => {
      const date = `2026-${String(index + 1).padStart(2, "0")}-05`;
      expect(formatDayLabel(date, "2026-12-31", "ru", RU)).toBe(`5 ${name}`);
      expect(formatDayLabel(date, "2026-12-31", "en", EN)).toBe(`${en[index]} 5`);
    });
  });

  it("високосный день", () => {
    expect(formatDayLabel("2024-02-29", today, "ru", RU)).toBe("29 февраля 2024");
  });

  it("запись в 00:30 стоит под вчерашней меткой, когда её journal_date — вчера (граница дня 04:00 считает Rust)", () => {
    expect(formatDayLabel("2026-10-19", "2026-10-20", "ru", RU)).toBe("Вчера");
  });

  it("не дата — строка как есть", () => {
    expect(formatDayLabel("вчера-ish", today, "ru", RU)).toBe("вчера-ish");
    expect(formatDayLabel("2026-10-20", "", "ru", RU)).toBe("2026-10-20");
  });
});

describe("continuesSeries", () => {
  const at = (journalDate: string, minutes: number) => ({ journalDate, createdAt: minutes * 60_000 });

  it("серия: пауза меньше 5 минут в тот же день", () => {
    expect(continuesSeries(at("2026-10-20", 0), at("2026-10-20", 2))).toBe(true);
    expect(continuesSeries(at("2026-10-20", 0), at("2026-10-20", 4))).toBe(true);
  });

  it("ровно 5 минут и больше — новая серия", () => {
    expect(SERIES_PAUSE_MS).toBe(300_000);
    expect(continuesSeries(at("2026-10-20", 0), at("2026-10-20", 5))).toBe(false);
    expect(continuesSeries(at("2026-10-20", 0), at("2026-10-20", 140))).toBe(false);
  });

  it("другой journal_date — серия не продолжается, даже если пауза мала", () => {
    expect(continuesSeries(at("2026-10-19", 0), at("2026-10-20", 1))).toBe(false);
  });

  it("сообщения не по порядку не образуют серию", () => {
    expect(continuesSeries(at("2026-10-20", 5), at("2026-10-20", 3))).toBe(false);
  });
});
