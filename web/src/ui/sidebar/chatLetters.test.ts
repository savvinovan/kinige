import { describe, expect, it } from "vitest";
import { TINTS, firstLetter, hasEmoji, tintForId, tintOf } from "./chatLetters";

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
    // «е» + U+0308 (комбинируемая диереза) и «a» + U+0301 (акут): запись из двух кодовых точек не рвётся.
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

describe("tintForId: цвет плитки из идентификатора", () => {
  it("один и тот же идентификатор всегда даёт один цвет", () => {
    for (const id of ["a", "inbox", "0b4f7c2e-5d7e-4f38-9a41-6e0f3f8f2c11"]) expect(tintForId(id)).toBe(tintForId(id));
  });

  it("известные значения не меняются (иначе чаты перекрасятся после обновления)", () => {
    expect(tintForId("")).toBe("teal");
    expect(tintForId("a")).toBe("pink");
  });

  it("цвета разные и покрывают палитру на разумной выборке", () => {
    const seen = new Set(Array.from({ length: 200 }, (_, index) => tintForId(`chat-${index}`)));
    expect(seen.size).toBe(TINTS.length);
  });

  it("всегда один из цветов палитры, в том числе для мусора", () => {
    for (const id of ["", "💬", "а".repeat(10_000), "\u0000", "\uD800"]) expect(TINTS).toContain(tintForId(id));
  });
});

describe("tintOf: заданный цвет главнее выведенного", () => {
  it("используется tint, если он есть", () => {
    expect(tintOf({ id: "a", tint: "violet" })).toBe("violet");
  });

  it("иначе цвет выводится из идентификатора", () => {
    expect(tintOf({ id: "a" })).toBe(tintForId("a"));
  });
});
