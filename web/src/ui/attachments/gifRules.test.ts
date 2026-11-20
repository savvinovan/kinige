import { describe, expect, it } from "vitest";
import { GIF_AUTOPLAY_MAX_BYTES, GIF_AUTOPLAY_MAX_PIXELS, gifMode, isGif } from "./gifRules";
import { FULL_MAX_BYTES } from "./mediaUrl";

const MIB = 1024 * 1024;
const gif = (byteSize: number, width: number | null, height: number | null) => ({ byteSize, width, height });

describe("gifMode: autoplay needs at most 8 MiB and 2,000,000 pixels", () => {
  it("the numbers are the spec's", () => {
    expect(GIF_AUTOPLAY_MAX_BYTES).toBe(8 * MIB);
    expect(GIF_AUTOPLAY_MAX_PIXELS).toBe(2_000_000);
    expect(FULL_MAX_BYTES).toBe(64 * MIB);
  });

  it("the byte edge: 8 MiB plays on its own, one byte more waits for a click", () => {
    expect(gifMode(gif(8 * MIB, 100, 100))).toBe("autoplay");
    expect(gifMode(gif(8 * MIB + 1, 100, 100))).toBe("click");
  });

  it("the pixel edge: 2,000,000 plays on its own, one pixel more waits for a click", () => {
    expect(gifMode(gif(1000, 2000, 1000))).toBe("autoplay");
    expect(gifMode(gif(1000, 2000, 1000 + 1))).toBe("click");
    expect(gifMode(gif(1000, 1_000_001, 2))).toBe("click");
  });

  it("up to 64 MiB a click plays it; over 64 MiB no `full` is served, so only the viewer is left", () => {
    expect(gifMode(gif(64 * MIB, 100, 100))).toBe("click");
    expect(gifMode(gif(64 * MIB + 1, 100, 100))).toBe("viewer");
    expect(gifMode(gif(64 * MIB + 1, null, null))).toBe("viewer");
  });

  it("a size that is not known (or nonsense) is never autoplayed", () => {
    expect(gifMode(gif(1000, null, null))).toBe("click");
    expect(gifMode(gif(1000, 100, null))).toBe("click");
    expect(gifMode(gif(1000, 0, 100))).toBe("click");
    expect(gifMode(gif(1000, -5, 100))).toBe("click");
  });
});

describe("isGif", () => {
  it("an image with the type image/gif, in any case; nothing else", () => {
    expect(isGif({ kind: "image", mime: "image/gif" })).toBe(true);
    expect(isGif({ kind: "image", mime: "IMAGE/GIF" })).toBe(true);
    expect(isGif({ kind: "image", mime: "image/webp" })).toBe(false);
    expect(isGif({ kind: "file", mime: "image/gif" })).toBe(false);
    expect(isGif({ kind: "image", mime: "" })).toBe(false);
  });
});
