import { describe, expect, it } from "vitest";
import { demoEntry } from "../../api/demoData";
import { type FeedMessage, buildFeed } from "./buildFeed";

const msg = (id: string, journalDate: string, time: string): FeedMessage => demoEntry(id, journalDate, time, id);

describe("buildFeed", () => {
  it("пусто: ни меток, ни сообщений", () => {
    expect(buildFeed([])).toEqual([]);
  });

  it("одна метка на journal_date, без повторов, в порядке (journal_date, created_at, id)", () => {
    const items = buildFeed([
      msg("c", "2026-10-20", "09:12"),
      msg("a", "2025-03-14", "09:12"),
      msg("b", "2026-10-19", "08:40"),
      msg("d", "2026-10-20", "11:30"),
    ]);
    expect(items.map((i) => (i.kind === "day" ? `# ${i.journalDate}` : i.message.id))).toEqual([
      "# 2025-03-14",
      "a",
      "# 2026-10-19",
      "b",
      "# 2026-10-20",
      "c",
      "d",
    ]);
  });

  it("запись после полуночи с journal_date вчера остаётся в вчерашнем дне, а не открывает новый", () => {
    // 00:30 по часам, но журнальный день — 19-е: идёт сразу за вечерними записями 19-го.
    const night = { ...msg("night", "2026-10-19", "21:30"), createdAt: Date.UTC(2026, 9, 20, 0, 30) };
    const items = buildFeed([msg("evening", "2026-10-19", "21:00"), night, msg("morning", "2026-10-20", "09:00")]);
    expect(items.map((i) => (i.kind === "day" ? `# ${i.journalDate}` : i.message.id))).toEqual([
      "# 2026-10-19",
      "evening",
      "night",
      "# 2026-10-20",
      "morning",
    ]);
  });

  it("серия: пауза меньше 5 минут; первое сообщение дня и после паузы — не в серии", () => {
    const items = buildFeed([
      msg("a", "2026-10-20", "09:12"),
      msg("b", "2026-10-20", "09:14"),
      msg("c", "2026-10-20", "09:18"),
      msg("d", "2026-10-20", "11:30"),
    ]);
    const series = items.flatMap((i) => (i.kind === "message" ? [[i.message.id, i.inSeries] as const] : []));
    expect(series).toEqual([
      ["a", false],
      ["b", true],
      ["c", true], // 9:14 -> 9:18: четыре минуты
      ["d", false],
    ]);
  });

  it("через границу journal_date серия не тянется, даже если пауза меньше 5 минут", () => {
    const a = msg("a", "2026-10-19", "23:58");
    const b = { ...msg("b", "2026-10-20", "00:01") };
    const items = buildFeed([a, b]);
    expect(items.flatMap((i) => (i.kind === "message" ? [i.inSeries] : []))).toEqual([false, false]);
    expect(items.filter((i) => i.kind === "day")).toHaveLength(2);
  });

  it("равное время создания — порядок по id", () => {
    const items = buildFeed([msg("b", "2026-10-20", "09:00"), msg("a", "2026-10-20", "09:00")]);
    expect(items.flatMap((i) => (i.kind === "message" ? [i.message.id] : []))).toEqual(["a", "b"]);
  });

  it("не меняет переданный массив", () => {
    const input = [msg("b", "2026-10-20", "10:00"), msg("a", "2026-10-20", "09:00")];
    buildFeed(input);
    expect(input.map((m) => m.id)).toEqual(["b", "a"]);
  });
});
