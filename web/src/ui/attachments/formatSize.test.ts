import { describe, expect, it } from "vitest";
import { formatSize } from "./formatSize";

const MEGABYTE = 1024 * 1024;

// The unit names come from the engine's ICU data (Node here, WebKit / WebView2 in the app): "kB" or "KB" for kilobytes depending on the data,
// so the tests allow both spellings and pin the number part and the separator.
describe("formatSize", () => {
  it("английский: точка и короткие единицы", () => {
    expect(formatSize(2.4 * MEGABYTE, "en")).toBe("2.4 MB");
    expect(formatSize(1536, "en")).toMatch(/^1\.5 [kK]B$/);
    expect(formatSize(3 * 1024 * MEGABYTE, "en")).toBe("3.0 GB");
  });

  it("русский: запятая и единицы по-русски", () => {
    expect(formatSize(2.4 * MEGABYTE, "ru")).toMatch(/^2,4\s(МБ|Мбайт)$/);
    expect(formatSize(1536, "ru")).toMatch(/^1,5\s(кБ|КБ|Кбайт)$/);
  });

  it("до килобайта целые байты", () => {
    expect(formatSize(0, "en")).toMatch(/^0\s?B$/);
    expect(formatSize(512, "en")).toMatch(/^512\s?B$/);
    expect(formatSize(512, "ru")).toMatch(/^512\s(Б|байт)$/);
  });

  it("значение, которое округляется до 1024, переходит в следующую единицу", () => {
    expect(formatSize(1024 * 1024 - 1, "en")).toBe("1.0 MB");
  });

  it("мусор не ломает строку", () => {
    for (const garbage of [-5, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(formatSize(garbage, "en")).toMatch(/^0\s?B$/);
    }
  });
});
