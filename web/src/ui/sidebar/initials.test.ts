import { describe, expect, it } from "vitest";
import { initials } from "./initials";

describe("initials", () => {
  it("пустое имя — запасное значение (имя по умолчанию из переводов: «Я» / «Me»)", () => {
    expect(initials("", "Я")).toBe("Я");
    expect(initials("   ", "Я")).toBe("Я");
    expect(initials("", "M")).toBe("M");
  });

  it("одно слово — одна буква, заглавная", () => {
    expect(initials("Я", "Я")).toBe("Я");
    expect(initials("саша", "Я")).toBe("С");
  });

  it("два и больше слов — первые буквы первых двух", () => {
    expect(initials("Александр Саввинов", "Я")).toBe("АС");
    expect(initials("мария иванова петрова", "Я")).toBe("МИ");
    expect(initials("  Ivan   Petrov ", "Я")).toBe("IP");
  });

  it("эмодзи и составные символы не рвутся", () => {
    expect(initials("🦊 Лис", "Я")).toBe("🦊Л");
  });
});
