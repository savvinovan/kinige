/** The columns of the Stickers tab grid and of the GIFs tab grid. The same numbers are the tokens `sticker.columns` and `sticker.gif-columns` (a test compares). */
export const STICKER_COLUMNS = 5;
export const GIF_COLUMNS = 3;

/** A place in a grid made of sections (a pack is a section): the section and the index inside it. */
export interface GridPlace {
  section: number;
  index: number;
}

/**
 * Where an arrow key, Home or End takes the focus in a grid of sections, each laid out in `columns` columns and starting on a row of its own. Left
 * and right walk the cells in reading order across sections; up and down keep the column (clamped to a shorter last row) and cross into the next or
 * previous section. At the first and last cell nothing moves (`null`), like the emoji grid. `sizes` is the number of cells of each section.
 */
export function moveInGrid(sizes: readonly number[], columns: number, from: GridPlace, key: string): GridPlace | null {
  const size = (section: number) => sizes[section] ?? 0;
  const column = from.index % columns;
  const row = Math.floor(from.index / columns);
  const clamp = (section: number, index: number): GridPlace => ({ section, index: Math.min(index, size(section) - 1) });
  switch (key) {
    case "ArrowRight": {
      if (from.index + 1 < size(from.section)) return { section: from.section, index: from.index + 1 };
      const next = nextSection(sizes, from.section, 1);
      return next === null ? null : { section: next, index: 0 };
    }
    case "ArrowLeft": {
      if (from.index > 0) return { section: from.section, index: from.index - 1 };
      const previous = nextSection(sizes, from.section, -1);
      return previous === null ? null : { section: previous, index: size(previous) - 1 };
    }
    case "ArrowDown": {
      const below = (row + 1) * columns + column;
      if (below < size(from.section)) return { section: from.section, index: below };
      // A shorter last row below: its last cell is the nearest one in that direction.
      if (Math.floor((size(from.section) - 1) / columns) > row) return clamp(from.section, size(from.section) - 1);
      const next = nextSection(sizes, from.section, 1);
      return next === null ? null : clamp(next, column);
    }
    case "ArrowUp": {
      if (row > 0) return { section: from.section, index: from.index - columns };
      const previous = nextSection(sizes, from.section, -1);
      if (previous === null) return null;
      return clamp(previous, Math.floor((size(previous) - 1) / columns) * columns + column);
    }
    case "Home": {
      const first = nextSection(sizes, -1, 1);
      return first === null ? null : { section: first, index: 0 };
    }
    case "End": {
      const last = nextSection(sizes, sizes.length, -1);
      return last === null ? null : { section: last, index: size(last) - 1 };
    }
    default:
      return null;
  }
}

/** The closest section after (`step` 1) or before (`step` -1) `from` that has cells. */
function nextSection(sizes: readonly number[], from: number, step: 1 | -1): number | null {
  for (let section = from + step; section >= 0 && section < sizes.length; section += step) {
    if ((sizes[section] ?? 0) > 0) return section;
  }
  return null;
}
