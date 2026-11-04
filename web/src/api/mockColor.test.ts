import { describe, expect, it } from "vitest";
import { INBOX_ID } from "./KinigeApi";
import { automaticColor, resolveColor } from "./mockColor";

// The same pairs as `VECTORS` in `crates/kinige-domain/src/chat_color.rs`, taken from the former `tintForId` of the frontend: the
// Rust rule and this emulation must agree, and no chat changed its colour when the colour moved to Rust.
const VECTORS: readonly [string, string][] = [
  ["", "teal"],
  ["a", "pink"],
  ["0b4f7c2e-5d7e-4f38-9a41-6e0f3f8f2c11", "green"],
  ["3f1c9a52-8b6e-4d07-a2c4-17e5d9b0c3a8", "teal"],
  ["7a2d5e90-1c3b-4f68-8e47-b0a9d2c61f35", "orange"],
  ["c8e04b17-9a5d-4c32-b6f1-52d7a83e0c94", "pink"],
  ["1d6f8a3b-2e47-4b90-9c5a-8f0e7d34b261", "teal"],
  ["e5a79c04-6d18-4a2f-b3e0-94c1f5d72a86", "teal"],
  ["92b3d7e1-4f05-4c8a-a6d9-0e1b58c3f470", "violet"],
  ["5c0e1f86-a7d3-4429-8b12-d6e9304a7fc5", "teal"],
];

describe("automaticColor: эмуляция правила из Rust", () => {
  it("общие векторы совпадают с Rust", () => {
    for (const [id, expected] of VECTORS) expect(automaticColor(id), id).toBe(expected);
  });

  it("никогда не серый и покрывает шесть оттенков", () => {
    const seen = new Set(Array.from({ length: 200 }, (_, index) => automaticColor(`chat-${index}`)));
    expect(seen.size).toBe(6);
    expect(seen.has("grey")).toBe(false);
  });

  it("мусорный идентификатор не падает", () => {
    for (const id of ["💬", "а".repeat(10_000), "\u0000", "\uD800"]) expect(automaticColor(id)).not.toBe("grey");
  });
});

describe("resolveColor", () => {
  it("Входящие всегда серые, выбор главнее автоматического, null — автоматический", () => {
    expect(resolveColor(INBOX_ID, "pink")).toBe("grey");
    expect(resolveColor("a", "violet")).toBe("violet");
    expect(resolveColor("a", null)).toBe("pink");
  });
});
