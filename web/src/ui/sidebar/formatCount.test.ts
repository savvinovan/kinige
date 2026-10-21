import { describe, expect, it } from "vitest";
import { formatCount } from "./formatCount";

// Группы разрядов и «тыс.» ставит Intl языка интерфейса (ADR-0005); пробелы в нём неразрывные, поэтому сравниваем без них.
const plain = (text: string) => text.replace(/\s/g, " ");

describe("formatCount (ru)", () => {
  it("ноль и нечисловые значения: бейдж скрыт", () => {
    expect(formatCount(0, "ru")).toBe("");
    expect(formatCount(-3, "ru")).toBe("");
    expect(formatCount(Number.NaN, "ru")).toBe("");
    expect(formatCount(Number.POSITIVE_INFINITY, "ru")).toBe("");
  });

  it("до тысячи: как есть", () => {
    expect(formatCount(1, "ru")).toBe("1");
    expect(formatCount(8, "ru")).toBe("8");
    expect(formatCount(999, "ru")).toBe("999");
  });

  it("от 1 000 до 9 999: с группой разрядов", () => {
    expect(plain(formatCount(1_000, "ru"))).toBe("1 000");
    expect(plain(formatCount(1_234, "ru"))).toBe("1 234");
    expect(plain(formatCount(9_999, "ru"))).toBe("9 999");
  });

  it("разряды отделены узким неразрывным пробелом U+202F, как в D1", () => {
    expect(formatCount(1_000, "ru")).toBe("1 000");
    expect(formatCount(9_999, "ru")).toBe("9 999");
    expect(formatCount(1_234, "en")).toBe("1,234");
  });

  it("от 10 000: «12 тыс.», дробная часть отбрасывается, «1000 тыс.» не бывает", () => {
    expect(plain(formatCount(10_000, "ru"))).toBe("10 тыс.");
    expect(plain(formatCount(12_345, "ru"))).toBe("12 тыс.");
    expect(plain(formatCount(99_999, "ru"))).toBe("99 тыс.");
    expect(plain(formatCount(999_999, "ru"))).toBe("999 тыс.");
  });

  it("от миллиона: «1,2 млн», до 10 млн — одна цифра после запятой (отбрасывается, а не округляется)", () => {
    expect(plain(formatCount(1_000_000, "ru"))).toBe("1 млн");
    expect(plain(formatCount(1_234_567, "ru"))).toBe("1,2 млн");
    expect(plain(formatCount(1_999_999, "ru"))).toBe("1,9 млн");
    expect(plain(formatCount(9_999_999, "ru"))).toBe("9,9 млн");
    expect(plain(formatCount(12_345_678, "ru"))).toBe("12 млн");
  });

  it("дробное число отбрасывается", () => {
    expect(formatCount(7.9, "ru")).toBe("7");
  });
});

describe("formatCount (en)", () => {
  it("«1,234», «12K», «1.2M»: те же правила отбрасывания", () => {
    expect(formatCount(7, "en")).toBe("7");
    expect(formatCount(1_234, "en")).toBe("1,234");
    expect(formatCount(9_999, "en")).toBe("9,999");
    expect(formatCount(12_345, "en")).toBe("12K");
    expect(formatCount(99_999, "en")).toBe("99K");
    expect(formatCount(999_999, "en")).toBe("999K");
    expect(formatCount(1_999_999, "en")).toBe("1.9M");
    expect(formatCount(12_345_678, "en")).toBe("12M");
    expect(formatCount(0, "en")).toBe("");
  });
});
