import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { TALL_PICTURE_ASPECT } from "./PendingTiles";

// What jsdom does not compute: the keyboard focus ring of a tile must not be clipped (PRD §12.2), two full lines of tiles must fit, and the
// "tall picture" threshold of the component is the token's number. The pixels are checked in a real browser by tests/visual/pending-tiles.spec.ts.
const css = (name: string) => readFileSync(join(import.meta.dirname, name), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
const rule = (source: string, selector: string) => new RegExp(`(?:^|\\})\\s*${selector.replace(/[.]/g, "\\.")}\\s*\\{([^}]*)\\}`).exec(source)?.[1] ?? "";
const tokens = JSON.parse(readFileSync(join(import.meta.dirname, "../../../../design/tokens/attachment.json"), "utf8")) as {
  layout: { attachment: Record<string, { $value: number | string }> };
};

describe("плитки над полем: правила CSS", () => {
  const tiles = css("./pending-tiles.css");
  const composer = css("../composer/composer.css");

  it("плитка не обрезает содержимое: кольца фокуса кнопки фото и крестика рисуются снаружи своих коробок", () => {
    const tile = rule(tiles, ".pending-tile");
    expect(tile).toContain("position: relative");
    expect(tile).not.toMatch(/overflow(-[xy])?\s*:\s*(hidden|clip|auto|scroll)/);
    // The picture is clipped by its own area (the open button), whose ring is outside its box and is not cut by its own overflow.
    expect(rule(tiles, ".pending-tile__open")).toMatch(/overflow\s*:\s*hidden/);
    // No other rule on the way (the list and its items) clips either.
    expect(rule(tiles, ".pending-tiles")).not.toMatch(/overflow/);
    expect(rule(tiles, ".pending-tiles__item")).not.toMatch(/overflow/);
  });

  it("область плиток оставляет место под всё кольцо (ширина + зазор) со всех сторон и считает его в предельной высоте", () => {
    const area = rule(composer, ".composer__tiles");
    expect(area).toMatch(/--composer-tiles-ring-room:\s*calc\(var\(--focus-width\)\s*\+\s*var\(--focus-offset\)\)/);
    expect(area).toMatch(/padding:\s*var\(--composer-tiles-ring-room\)/);
    // Two full lines: 2 heights + the gap + the padding twice (the box is border-box), or the share of the window, whichever is less.
    const cap = area.replace(/\s+/g, " ");
    expect(cap).toContain("2 * var(--layout-attachment-pending-image-height)");
    expect(cap).toContain("var(--layout-attachment-pending-gap)");
    expect(cap).toContain("2 * var(--composer-tiles-ring-room)");
    expect(cap).toContain("var(--layout-attachment-pending-area-max-height)");
  });

  it("кольцо кнопки фото стоит обычный зазор от внешнего края плитки (а не от области внутри рамки)", () => {
    const ring = rule(tiles, ".pending-tile__open:focus-visible");
    expect(ring).toMatch(/outline-offset:\s*calc\(var\(--focus-offset\)\s*\+\s*var\(--border-width-hairline\)\)/);
  });

  it("высокое фото: уже плитки 3:4 — обрезка сверху; число в компоненте то же, что в токене", () => {
    expect(rule(tiles, ".pending-tile--tall .pending-tile__image")).toMatch(/object-fit:\s*cover[\s\S]*object-position:\s*top/);
    expect(tokens.layout.attachment["pending-image-min-aspect"]?.$value).toBe(TALL_PICTURE_ASPECT);
    expect(rule(tiles, ".pending-tile--image")).toContain("var(--layout-attachment-pending-image-min-aspect)");
  });

  it("предельная ширина плитки 16:9 при высоте 144, а доля окна даёт две полные строки при стандартной высоте окна", () => {
    const height = parseFloat(String(tokens.layout.attachment["pending-image-height"]?.$value));
    const hairline = 1;
    const maxWidth = parseFloat(String(tokens.layout.attachment["pending-image-max-width"]?.$value));
    // 16:9 of the picture area (the height without the hairlines) plus the hairlines fits under the cap.
    expect((height - 2 * hairline) * (16 / 9) + 2 * hairline).toBeLessThanOrEqual(maxWidth);
    const share = parseFloat(String(tokens.layout.attachment["pending-area-max-height"]?.$value));
    const gap = 0.5 * 13;
    const ring = 4;
    // The token is a share of the window height (the number before its unit); the default window is 800 high.
    expect(0.01 * share * 800).toBeGreaterThanOrEqual(2 * height + gap + 2 * ring);
  });
});
