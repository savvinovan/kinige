import { describe, expect, it } from "vitest";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { demoPhoto } from "./demo";
import { stickerOf } from "./stickerRule";

const sticker = (part: Partial<AttachmentView> = {}): AttachmentView => ({
  ...demoPhoto("s1", "sticker.webp", 512, 512, { transparent: true }),
  sticker: true,
  stickerEmoji: "\u{1F63A}",
  ...part,
});

describe("stickerOf: the display rule of kinige_domain::attachment::sticker_of", () => {
  it("an empty body, one uncovered sticker: drawn as a sticker", () => {
    const only = sticker();
    expect(stickerOf("", [only])).toBe(only);
  });

  it("a body that is not empty (an entry edited after the import): an ordinary picture", () => {
    expect(stickerOf("a note", [sticker()])).toBeNull();
    // Rust compares with `is_empty`, so a body of spaces is a body.
    expect(stickerOf(" ", [sticker()])).toBeNull();
  });

  it("a covered sticker is the 7b cover, not a sticker", () => {
    expect(stickerOf("", [sticker({ spoiler: true })])).toBeNull();
  });

  it("not a sticker, no attachments, or more than one attachment", () => {
    expect(stickerOf("", [sticker({ sticker: false })])).toBeNull();
    expect(stickerOf("", [])).toBeNull();
    expect(stickerOf("", undefined)).toBeNull();
    expect(stickerOf("", [sticker(), sticker({ id: "s2" })])).toBeNull();
  });

  it("a sticker without an emoji is still a sticker", () => {
    expect(stickerOf("", [sticker({ stickerEmoji: null })])).not.toBeNull();
  });
});
