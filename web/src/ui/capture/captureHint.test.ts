import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// KINIG-153: ⌘Enter does nothing while the recording bar (or its question) stands in for the field, so the hint "⌘Enter — append" is hidden
// then, but it keeps its line: removing it would make the window one line shorter the moment recording starts. jsdom cannot evaluate `:has`,
// so the stylesheet is read.
const css = readFileSync(join(import.meta.dirname, "capture.css"), "utf8");

describe("the append hint while the recording bar is up", () => {
  it("is hidden with `visibility`, which keeps its line and takes it out of what a screen reader reads", () => {
    const rule = /\.capture-panel:has\(\.recording-bar\)\s+\.composer__hint\s*\{([^}]*)\}/.exec(css);
    expect(rule).not.toBeNull();
    expect(rule?.[1]).toMatch(/visibility:\s*hidden/);
    // Not `display: none` and not `height: 0`: those would move the field.
    expect(rule?.[1]).not.toMatch(/display:|height:/);
  });
});
