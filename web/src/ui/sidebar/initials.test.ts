import { describe, expect, it } from "vitest";
import { DEFAULT_PROFILE_NAME, initials } from "./initials";

describe("initials", () => {
  it("по умолчанию «Я»", () => {
    expect(DEFAULT_PROFILE_NAME).toBe("Я");
    expect(initials("")).toBe("Я");
    expect(initials("   ")).toBe("Я");
  });

  it("одно слово — одна буква, заглавная", () => {
    expect(initials("Я")).toBe("Я");
    expect(initials("саша")).toBe("С");
  });

  it("два и больше слов — первые буквы первых двух", () => {
    expect(initials("Александр Саввинов")).toBe("АС");
    expect(initials("мария иванова петрова")).toBe("МИ");
    expect(initials("  Ivan   Petrov ")).toBe("IP");
  });

  it("эмодзи и составные символы не рвутся", () => {
    expect(initials("🦊 Лис")).toBe("🦊Л");
  });
});
