import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// KINIG-76 and KINIG-150: jsdom does not load CSS, so the rules are checked against the files; the real geometry at 640 x 400 is measured by
// `tests/visual/main-window-size.spec.ts`.
const read = (path: string) => readFileSync(resolve(__dirname, path), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

function rules(source: string): Map<string, string> {
  const result = new Map<string, string>();
  for (const match of source.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const [selector = "", body = ""] = [match[1], match[2]];
    for (const one of selector.split(",")) result.set(one.trim(), `${result.get(one.trim()) ?? ""}${body}`);
  }
  return result;
}

describe("the main window's document", () => {
  const css = rules(read("main-window.css"));

  it("does not scroll while the journal is shown, and the recovery screens may", () => {
    expect(css.get('html[data-window="main"]:has(.main-window)')).toMatch(/overflow\s*:\s*hidden/);
    expect(css.get('html[data-window="main"]:has(.main-window) body')).toMatch(/overflow\s*:\s*hidden/);
    expect(css.get('html[data-window="main"]')).toBeUndefined();
  });

  it("main.tsx marks the main window", () => {
    expect(readFileSync(resolve(__dirname, "../../main.tsx"), "utf8")).toMatch(/dataset\.window\s*=\s*"main"/);
  });

  it("the top bar holds the switch's height and the title is one line cut with an ellipsis", () => {
    expect(css.get(".main-window__topbar")).toMatch(/min-height\s*:\s*calc\(var\(--layout-action-size\)/);
    const title = css.get(".main-window__title") ?? "";
    expect(title).toMatch(/white-space\s*:\s*nowrap/);
    expect(title).toMatch(/text-overflow\s*:\s*ellipsis/);
    expect(title).toMatch(/min-width\s*:\s*0/);
  });

  it("the reply strip's label shrinks, so it cannot push the cross out of a narrow column", () => {
    const label = rules(read("../reply/reply.css")).get(".reply-strip__label") ?? "";
    expect(label).toMatch(/flex\s*:\s*0 1 auto/);
    expect(label).toMatch(/min-width\s*:\s*0/);
    expect(label).toMatch(/text-overflow\s*:\s*ellipsis/);
  });
});
