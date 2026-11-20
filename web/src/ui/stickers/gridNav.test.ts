import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { GIF_COLUMNS, STICKER_COLUMNS, moveInGrid } from "./gridNav";

const at = (section: number, index: number) => ({ section, index });

describe("the grid columns are the tokens", () => {
  it("STICKER_COLUMNS and GIF_COLUMNS equal sticker.columns and sticker.gif-columns", () => {
    const css = readFileSync(join(import.meta.dirname, "..", "tokens.css"), "utf8");
    expect(css).toContain(`--sticker-columns: ${STICKER_COLUMNS};`);
    expect(css).toContain(`--sticker-gif-columns: ${GIF_COLUMNS};`);
  });
});

describe("moveInGrid", () => {
  // Three sections: 12 cells (rows of 5, 5, 2), 3 cells, 7 cells.
  const sizes = [12, 3, 7];

  it("left and right walk the reading order and cross sections", () => {
    expect(moveInGrid(sizes, 5, at(0, 3), "ArrowRight")).toEqual(at(0, 4));
    expect(moveInGrid(sizes, 5, at(0, 11), "ArrowRight")).toEqual(at(1, 0));
    expect(moveInGrid(sizes, 5, at(1, 0), "ArrowLeft")).toEqual(at(0, 11));
    expect(moveInGrid(sizes, 5, at(2, 6), "ArrowRight")).toBeNull();
    expect(moveInGrid(sizes, 5, at(0, 0), "ArrowLeft")).toBeNull();
  });

  it("down keeps the column and clamps to a shorter last row", () => {
    expect(moveInGrid(sizes, 5, at(0, 2), "ArrowDown")).toEqual(at(0, 7));
    // Column 4 of the second row; the last row has only two cells: the nearest is the last one.
    expect(moveInGrid(sizes, 5, at(0, 9), "ArrowDown")).toEqual(at(0, 11));
  });

  it("down from the last row goes into the next section on its first row, same column, clamped", () => {
    expect(moveInGrid(sizes, 5, at(0, 11), "ArrowDown")).toEqual(at(1, 1));
    expect(moveInGrid(sizes, 5, at(1, 2), "ArrowDown")).toEqual(at(2, 2));
    expect(moveInGrid(sizes, 5, at(0, 10), "ArrowDown")).toEqual(at(1, 0));
    expect(moveInGrid(sizes, 5, at(2, 6), "ArrowDown")).toBeNull();
  });

  it("up goes to the previous row, or to the last row of the previous section at the same column", () => {
    expect(moveInGrid(sizes, 5, at(0, 7), "ArrowUp")).toEqual(at(0, 2));
    expect(moveInGrid(sizes, 5, at(1, 2), "ArrowUp")).toEqual(at(0, 11));
    expect(moveInGrid(sizes, 5, at(2, 1), "ArrowUp")).toEqual(at(1, 1));
    expect(moveInGrid(sizes, 5, at(0, 3), "ArrowUp")).toBeNull();
  });

  it("empty sections are skipped; Home and End go to the edge cells", () => {
    expect(moveInGrid([2, 0, 3], 5, at(0, 1), "ArrowRight")).toEqual(at(2, 0));
    expect(moveInGrid([2, 0, 3], 5, at(2, 0), "ArrowLeft")).toEqual(at(0, 1));
    expect(moveInGrid([0, 4, 6], 5, at(2, 3), "Home")).toEqual(at(1, 0));
    expect(moveInGrid([0, 4, 6], 5, at(1, 0), "End")).toEqual(at(2, 5));
  });

  it("three columns (the GIFs tab) and other keys", () => {
    expect(moveInGrid([8], 3, at(0, 1), "ArrowDown")).toEqual(at(0, 4));
    expect(moveInGrid([8], 3, at(0, 7), "ArrowUp")).toEqual(at(0, 4));
    expect(moveInGrid([8], 3, at(0, 0), "a")).toBeNull();
    expect(moveInGrid([], 5, at(0, 0), "Home")).toBeNull();
  });
});
