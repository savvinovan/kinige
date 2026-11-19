import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { contrastRatio } from "../contrast";
import { MIN_RATIO } from "../contrastPairs";
import { palettes } from "../tokenSource";

// KINIG-132: behavior that is visible only in a real window (jsdom does not compute CSS): the rules of a frameless picture bubble must not
// disappear from the stylesheet, and the colors they use must keep their contrast.
const css = readFileSync(join(import.meta.dirname, "message-bleed.css"), "utf8");
const colors = palettes.graphite?.colors ?? {};

/** The declarations of the rule with exactly this selector (comments removed); empty when there is no such rule. */
function rule(selector: string): string {
  const plain = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`(?:^|\\})\\s*${escaped}\\s*\\{([^}]*)\\}`).exec(plain)?.[1] ?? "";
}

/** The palette color a `var(--color-...)` in a declaration points at. */
function colorOf(declarations: string, property: string): string {
  const name = new RegExp(`${property}:[^;]*var\\(--color-([a-z-]+)\\)`).exec(declarations)?.[1] ?? "";
  const role = Object.keys(colors).find((key) => key.replaceAll(".", "-") === name);
  return colors[role ?? ""] ?? "";
}

describe("message-bleed.css", () => {
  it("the time pill lets clicks through to the picture under it (the viewer opens on click)", () => {
    expect(rule(".message--bleed-pill .message__meta")).toMatch(/pointer-events:\s*none/);
  });

  it("control: the rule finder reads nothing for a selector that is not there", () => {
    expect(rule(".message--bleed-no-such-rule")).toBe("");
  });

  it("a missing or broken picture on a frameless bubble has a tile that shows against the feed", () => {
    const tile = colorOf(rule(".message--bleed-bare .preview-tile"), "background");
    expect(tile).not.toBe("");
    expect(contrastRatio(tile, colors["surface.paper"] ?? "")).toBeGreaterThanOrEqual(MIN_RATIO.decorative);
    // The tile text sits on it.
    expect(contrastRatio(colors["text.tertiary"] ?? "", tile)).toBeGreaterThanOrEqual(MIN_RATIO.text);
  });

  it("the usual tile (the window background) would not show against the feed: the reason for the rule above", () => {
    expect(contrastRatio(colors["surface.background"] ?? "", colors["surface.paper"] ?? "")).toBeLessThan(MIN_RATIO.decorative);
  });

  it("the ring of a highlighted frameless bubble is easy to see on the feed", () => {
    const ring = colorOf(rule(".message--bleed-bare.message--highlighted"), "outline");
    expect(ring).not.toBe("");
    expect(contrastRatio(ring, colors["surface.paper"] ?? "")).toBeGreaterThanOrEqual(MIN_RATIO.ui);
  });

  it("a bubble of pictures has a floor width, and a bubble with a pill and nothing else has the room its pill needs", () => {
    expect(rule(".message--bleed .message-attachments")).toMatch(/min-width:\s*var\(--layout-attachment-bleed-min-width\)/);
    for (const selector of [
      ".message--bleed-bare .message-attachments",
      ".message--bleed-bare.message--favorite .message-attachments",
      ".message--bleed-bare.message--agent .message-attachments",
      ".message--bleed-bare.message--agent.message--favorite .message-attachments",
    ]) {
      expect(rule(selector), selector).toContain("--layout-attachment-pill-inset");
    }
  });

  // The pixels themselves are sampled in a real browser (tests/visual/picture-bubble.spec.ts); these keep the two causes out of the stylesheet.
  it("a single picture covers its box, so no row of anything else shows at its edge", () => {
    expect(rule(".message--bleed .album--single .album__image")).toMatch(/object-fit:\s*cover/);
  });

  it("the bubble's fill is only beside a picture, never behind it: no background on the strip itself", () => {
    expect(rule(".message--bleed .message-attachments")).not.toMatch(/background/);
    // The two side columns of the grid, one rule for both.
    const sides = ".message--bleed .message-attachments::before,\n.message--bleed .message-attachments::after";
    expect(rule(sides)).toMatch(/background:\s*var\(--color-surface-message\)/);
  });
});
