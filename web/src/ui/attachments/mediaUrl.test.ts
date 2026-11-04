import { afterEach, describe, expect, it, vi } from "vitest";
import { demoFile, demoPhoto } from "./demo";
import { FULL_MAX_BYTES, canShowFull, mediaUrl } from "./mediaUrl";

afterEach(() => vi.restoreAllMocks());

describe("mediaUrl", () => {
  it("вне Windows: схема kinige-media", () => {
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15");
    expect(mediaUrl("thumb", "abc")).toBe("kinige-media://localhost/thumb/abc");
    expect(mediaUrl("full", "abc")).toBe("kinige-media://localhost/full/abc");
  });

  it("в Windows: http-источник kinige-media.localhost", () => {
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/130.0");
    expect(mediaUrl("thumb", "abc")).toBe("http://kinige-media.localhost/thumb/abc");
    expect(mediaUrl("full", "abc")).toBe("http://kinige-media.localhost/full/abc");
  });
});

describe("canShowFull", () => {
  const image = demoPhoto("a", "a.jpg", 100, 100);

  it("картинка известного формата до 64 МиБ", () => {
    expect(canShowFull(image)).toBe(true);
    expect(canShowFull({ ...image, mime: "image/png" })).toBe(true);
    expect(canShowFull({ ...image, mime: "IMAGE/WEBP" })).toBe(true);
    expect(canShowFull({ ...image, byteSize: FULL_MAX_BYTES })).toBe(true);
  });

  it("больше предела, другой формат, не картинка и пропавший файл: только миниатюра", () => {
    expect(canShowFull({ ...image, byteSize: FULL_MAX_BYTES + 1 })).toBe(false);
    expect(canShowFull({ ...image, mime: "image/svg+xml" })).toBe(false);
    expect(canShowFull({ ...image, mime: "image/heic" })).toBe(false);
    expect(canShowFull(demoFile("f", "a.pdf", "file", 10))).toBe(false);
    expect(canShowFull({ ...image, state: "missing" })).toBe(false);
  });
});
