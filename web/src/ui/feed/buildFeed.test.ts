import { describe, expect, it } from "vitest";
import { type FeedMessage, buildFeed } from "./buildFeed";

const at = (month: number, day: number, hour: number, minute: number, year = 2026) => new Date(year, month - 1, day, hour, minute);
const msg = (id: string, sentAt: Date): FeedMessage => ({ id, sentAt, markdown: id });
const now = at(10, 20, 17, 0);

describe("buildFeed", () => {
  it("пусто: ни меток, ни сообщений", () => {
    expect(buildFeed([], now)).toEqual([]);
  });

  it("одна метка на день, без повторов, в порядке времени", () => {
    const items = buildFeed(
      [msg("c", at(10, 20, 9, 12)), msg("a", at(3, 14, 9, 12, 2025)), msg("b", at(10, 19, 8, 40)), msg("d", at(10, 20, 11, 30))],
      now,
    );
    expect(items.filter((i) => i.kind === "day").map((i) => i.kind === "day" && i.label)).toEqual([
      "14 марта 2025",
      "Вчера",
      "Сегодня",
    ]);
    expect(items.map((i) => (i.kind === "day" ? `# ${i.label}` : i.message.id))).toEqual([
      "# 14 марта 2025",
      "a",
      "# Вчера",
      "b",
      "# Сегодня",
      "c",
      "d",
    ]);
  });

  it("серия: пауза меньше 5 минут; первое сообщение дня и после паузы — не в серии", () => {
    const items = buildFeed(
      [msg("a", at(10, 20, 9, 12)), msg("b", at(10, 20, 9, 14)), msg("c", at(10, 20, 9, 18)), msg("d", at(10, 20, 11, 30))],
      now,
    );
    const series = items.flatMap((i) => (i.kind === "message" ? [[i.message.id, i.inSeries] as const] : []));
    expect(series).toEqual([
      ["a", false],
      ["b", true],
      ["c", true], // 9:14 -> 9:18: четыре минуты
      ["d", false],
    ]);
  });

  it("через полночь серия не тянется, даже если пауза меньше 5 минут", () => {
    const items = buildFeed([msg("a", at(10, 19, 23, 58)), msg("b", at(10, 20, 0, 1))], now);
    expect(items.flatMap((i) => (i.kind === "message" ? [i.inSeries] : []))).toEqual([false, false]);
    expect(items.filter((i) => i.kind === "day")).toHaveLength(2);
  });

  it("не меняет переданный массив", () => {
    const input = [msg("b", at(10, 20, 10, 0)), msg("a", at(10, 20, 9, 0))];
    buildFeed(input, now);
    expect(input.map((m) => m.id)).toEqual(["b", "a"]);
  });
});
