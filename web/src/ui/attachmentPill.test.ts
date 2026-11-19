import { describe, expect, it } from "vitest";
import attachmentTokens from "../../../design/tokens/attachment.json";
import { contrastRatio } from "./contrast";
import { MIN_RATIO } from "./contrastPairs";
import { palettes } from "./tokenSource";

// KINIG-132: the time pill drawn on a picture (as in Telegram) is a translucent fill with light text on it. The picture shows through the
// fill, so the contrast is computed against the fill laid over a picture: the worst case for light text is a pure white picture.
const colors = palettes.graphite?.colors ?? {};
const fill = colors["attachment.pill-background"] ?? "";
const text = colors["attachment.pill-text"] ?? "";
const alpha = attachmentTokens.opacity["attachment-pill"].$value;

/** The fill at `opacity` laid over a gray picture of the given level (0 to 255): the color the text is drawn on. */
function overGray(level: number, opacity = alpha): string {
  const channel = (index: number) => Number.parseInt(fill.slice(index, index + 2), 16);
  const mixed = [1, 3, 5].map((index) => Math.round(channel(index) * opacity + level * (1 - opacity)));
  return `#${mixed.map((value) => value.toString(16).padStart(2, "0")).join("")}`;
}

describe("the time pill on a picture", () => {
  it("has a translucent fill: a picture shows through it", () => {
    expect(alpha).toBeGreaterThan(0);
    expect(alpha).toBeLessThan(1);
  });

  it("reads on a white picture, the worst case for light text", () => {
    expect(contrastRatio(text, overGray(255))).toBeGreaterThanOrEqual(MIN_RATIO.text);
  });

  it("reads on a black picture", () => {
    expect(contrastRatio(text, overGray(0))).toBeGreaterThanOrEqual(MIN_RATIO.text);
  });

  it("reads on every gray level between them", () => {
    for (let level = 0; level <= 255; level += 5) {
      expect(contrastRatio(text, overGray(level)), `picture level ${level}`).toBeGreaterThanOrEqual(MIN_RATIO.text);
    }
  });

  it("control: a fill at 0.3 opacity would fail on a white picture, so the checks above can fail", () => {
    expect(contrastRatio(text, overGray(255, 0.3))).toBeLessThan(MIN_RATIO.text);
  });
});
