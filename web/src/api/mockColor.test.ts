import { describe, expect, it } from "vitest";
import { INBOX_ID } from "./KinigeApi";
import { automaticColor, resolveColor } from "./mockColor";

// The pairs of id and colour that Rust and this emulation must agree on are in `fixtures/api-contract/chat-color.json`
// (`apiContract.test.ts` here, `crates/kinige-store/tests/api_contract/` there); these tests keep what the fixture does not say.
describe("automaticColor: эмуляция правила из Rust", () => {
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
