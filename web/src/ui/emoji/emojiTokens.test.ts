import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { EMOJI_COLUMNS } from "./emojiLayout";

describe("пикер эмодзи и токены", () => {
  it("число столбцов в коде равно токену emoji.columns", () => {
    const css = readFileSync(join(import.meta.dirname, "..", "tokens.css"), "utf8");
    expect(css).toContain(`--emoji-columns: ${EMOJI_COLUMNS};`);
  });
});
