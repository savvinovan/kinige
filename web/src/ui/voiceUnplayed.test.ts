import { describe, expect, it } from "vitest";
import { contrastRatio } from "./contrast";
import { MIN_RATIO } from "./contrastPairs";
import { palettes } from "./tokenSource";

// The unplayed bars of a voice note's waveform (slice 7a, round 3): a neutral gray with the largest step from the accent that the project's
// 2.0 minimum for a graphic on the message plate still allows. The minimum is not lowered; the gray is the darkest one that meets it.
const colors = palettes.graphite?.colors ?? {};
const unplayed = colors["voice.unplayed"] ?? "";
const plate = colors["surface.message"] ?? "";
const accent = colors["accent.default"] ?? "";

const darker = (hex: string) => {
  const channel = Math.max(0, Number.parseInt(hex.slice(1, 3), 16) - 1);
  const part = channel.toString(16).padStart(2, "0");
  return `#${part}${part}${part}`;
};

describe("voice.unplayed", () => {
  it("is a neutral gray", () => {
    expect(unplayed).toMatch(/^#([0-9a-f]{2})\1\1$/);
  });

  it("keeps the 2.0 graphic minimum on the message plate, which is not lowered", () => {
    expect(MIN_RATIO.graphic).toBe(2);
    expect(contrastRatio(unplayed, plate)).toBeGreaterThanOrEqual(MIN_RATIO.graphic);
  });

  it("is the darkest gray that meets it: one step darker fails, so the step from the accent is the largest allowed", () => {
    expect(contrastRatio(darker(unplayed), plate)).toBeLessThan(MIN_RATIO.graphic);
    expect(contrastRatio(accent, unplayed)).toBeGreaterThan(3.4);
  });

  it("stays visible on the highlighted plate", () => {
    expect(contrastRatio(unplayed, colors["highlight.background"] ?? "")).toBeGreaterThanOrEqual(MIN_RATIO.selection);
  });
});
