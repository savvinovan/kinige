import { describe, expect, it } from "vitest";
import { albumRows } from "./albumRows";

describe("albumRows", () => {
  it.each([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 25])("%i фото: ряды дают ровно столько, в рядах от одного до трёх", (count) => {
    const rows = albumRows(count);
    expect(rows.reduce((sum, row) => sum + row, 0)).toBe(count);
    for (const row of rows) {
      expect(row).toBeGreaterThanOrEqual(1);
      expect(row).toBeLessThanOrEqual(3);
    }
  });

  it("форма по числу, как в описании", () => {
    expect(albumRows(2)).toEqual([2]);
    expect(albumRows(3)).toEqual([1, 2]);
    expect(albumRows(4)).toEqual([2, 2]);
    expect(albumRows(10)).toEqual([2, 2, 3, 3]);
  });

  it("мусор: ноль и отрицательное число дают пусто", () => {
    expect(albumRows(0)).toEqual([]);
    expect(albumRows(-3)).toEqual([]);
  });
});
