import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(join(import.meta.dirname, "message-sticker.css"), "utf8");

/** The declarations of the first rule whose selector is exactly `selector`. */
function rule(selector: string): string {
  const start = css.indexOf(`${selector} {`);
  expect(start, selector).toBeGreaterThanOrEqual(0);
  return css.slice(start, css.indexOf("}", start));
}

describe("the sticker's box", () => {
  it("is as wide as the picture, so the time pill sits on the picture's corner and not on a wider row (a reply quote makes the row wide)", () => {
    const box = rule(".message--sticker .message__sticker");
    expect(box).toMatch(/position:\s*relative/);
    expect(box).toMatch(/width:\s*fit-content/);
  });
});
