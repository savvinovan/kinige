import { describe, expect, it } from "vitest";
import { firstLetter, hasEmoji } from "./chatLetters";

describe("hasEmoji: что считается «без эмодзи»", () => {
  it("настоящий эмодзи — есть", () => {
    for (const emoji of ["📚", "🏖️", "👨‍👩‍👧", "❤️"]) expect(hasEmoji(emoji), emoji).toBe(true);
  });

  it("пусто, пробелы, null и значок по умолчанию «💬» — нет", () => {
    for (const emoji of ["", "   ", null, undefined, "💬", " 💬 "]) expect(hasEmoji(emoji), String(emoji)).toBe(false);
  });
});

describe("firstLetter", () => {
  it("кириллица и латиница, в верхнем регистре", () => {
    expect(firstLetter("Проекты")).toBe("П");
    expect(firstLetter("мама")).toBe("М");
    expect(firstLetter("Linux notes")).toBe("L");
    expect(firstLetter("reading list")).toBe("R");
  });

  it("«Ё» и «ё» остаются «Ё», а не «Е»", () => {
    expect(firstLetter("Ёлка")).toBe("Ё");
    expect(firstLetter("ёлка")).toBe("Ё");
  });

  it("буква с комбинируемым знаком — одна графема целиком", () => {
    // "е" + U+0308 (combining diaeresis) and "a" + U+0301 (acute): a sequence of two code points is not split.
    expect(firstLetter("ёлка")).toBe("Ё");
    expect(firstLetter("ábc")).toBe("Á");
  });

  it("пробелы и знаки в начале пропускаются", () => {
    expect(firstLetter("   Мама")).toBe("М");
    expect(firstLetter("«Работа»")).toBe("Р");
    expect(firstLetter("- (draft) notes")).toBe("D");
    expect(firstLetter("\n\tidea")).toBe("I");
  });

  it("эмодзи в начале названия пропускается, берётся первая буква (суррогатные пары не рвутся)", () => {
    expect(firstLetter("🎉 Party plans")).toBe("P");
    expect(firstLetter("🏖️🌴 Отпуск")).toBe("О");
    expect(firstLetter("👨‍👩‍👧 Семья")).toBe("С");
  });

  it("цифра тоже годится", () => {
    expect(firstLetter("2025 план")).toBe("2");
  });

  it("«ß» в верхнем регистре даёт одну графему", () => {
    expect(firstLetter("ßtraße")).toBe("S");
  });

  it("без букв и цифр — «#»", () => {
    for (const title of ["", "   ", "!!!", "🎉", "…"]) expect(firstLetter(title), title).toBe("#");
  });

  it("мусорный ввод не падает: очень длинная строка, парные суррогаты, одинокий суррогат", () => {
    expect(firstLetter("а".repeat(100_000))).toBe("А");
    expect(firstLetter("\uD83D")).toBe("#");
    expect(firstLetter("😀x")).toBe("X");
  });
});
