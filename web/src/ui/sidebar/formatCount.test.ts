import { describe, expect, it } from "vitest";
import { THIN_NBSP, describeCount, formatCount } from "./formatCount";

describe("formatCount", () => {
  it("ноль и нечисловые значения: бейдж скрыт", () => {
    expect(formatCount(0)).toBe("");
    expect(formatCount(-3)).toBe("");
    expect(formatCount(Number.NaN)).toBe("");
    expect(formatCount(Number.POSITIVE_INFINITY)).toBe("");
  });

  it("до тысячи: как есть", () => {
    expect(formatCount(1)).toBe("1");
    expect(formatCount(8)).toBe("8");
    expect(formatCount(999)).toBe("999");
  });

  it("от 1 000 до 9 999: узкий неразрывный пробел между разрядами", () => {
    expect(formatCount(1_000)).toBe(`1${THIN_NBSP}000`);
    expect(formatCount(1_234)).toBe(`1${THIN_NBSP}234`);
    expect(formatCount(9_999)).toBe(`9${THIN_NBSP}999`);
    expect(THIN_NBSP).toBe(" ");
  });

  it("от 10 000: «12 тыс.», дробная часть отбрасывается, «1000 тыс.» не бывает", () => {
    expect(formatCount(10_000)).toBe("10 тыс.");
    expect(formatCount(12_345)).toBe("12 тыс.");
    expect(formatCount(99_999)).toBe("99 тыс.");
    expect(formatCount(999_999)).toBe("999 тыс.");
  });

  it("от миллиона: «1,2 млн», целые миллионы без «,0», от десяти миллионов без дробной части", () => {
    expect(formatCount(1_000_000)).toBe("1 млн");
    expect(formatCount(1_234_567)).toBe("1,2 млн");
    expect(formatCount(1_999_999)).toBe("1,9 млн");
    expect(formatCount(2_050_000)).toBe("2 млн");
    expect(formatCount(9_999_999)).toBe("9,9 млн");
    expect(formatCount(12_345_678)).toBe("12 млн");
  });

  it("дробные значения отбрасываются, а не округляются вверх", () => {
    expect(formatCount(999.9)).toBe("999");
    expect(formatCount(0.5)).toBe("");
  });
});

describe("describeCount (подсказка и диктор)", () => {
  it("число с узкими пробелами и склонением", () => {
    expect(describeCount(1)).toBe("1 запись");
    expect(describeCount(2)).toBe("2 записи");
    expect(describeCount(5)).toBe("5 записей");
    expect(describeCount(11)).toBe("11 записей");
    expect(describeCount(21)).toBe("21 запись");
    expect(describeCount(112)).toBe("112 записей");
    expect(describeCount(1_234)).toBe(`1${THIN_NBSP}234 записи`);
    expect(describeCount(12_345)).toBe(`12${THIN_NBSP}345 записей`);
    expect(describeCount(1_234_567)).toBe(`1${THIN_NBSP}234${THIN_NBSP}567 записей`);
  });
});
