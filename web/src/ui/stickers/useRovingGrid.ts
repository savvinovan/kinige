import { useRef, useState, type KeyboardEvent } from "react";
import { type GridPlace, moveInGrid } from "./gridNav";

const KEYS = new Set(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"]);

/**
 * A grid of buttons in sections with one Tab stop (roving tabindex), as the emoji grid has one for the keyboard: the arrows move by cell and by row
 * (`moveInGrid`), Home and End go to the first and last cell, Enter and Space press the focused button. `sizes` is the number of cells of each section.
 * Spread `cell(section, index)` on every button and `onKeyDown` on the element that holds them.
 */
export function useRovingGrid(sizes: readonly number[], columns: number) {
  const cells = useRef(new Map<string, HTMLButtonElement>());
  const [requested, setActive] = useState<GridPlace>({ section: 0, index: 0 });
  // The list can shrink (a pack was removed): a place that is no longer there falls back to the first cell.
  const first = Math.max(0, sizes.findIndex((size) => size > 0));
  const active = requested.index < (sizes[requested.section] ?? 0) ? requested : { section: first, index: 0 };

  const focusAt = (place: GridPlace) => {
    setActive(place);
    cells.current.get(`${place.section}:${place.index}`)?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (!KEYS.has(event.key) || !(event.target instanceof HTMLElement)) return;
    const section = Number(event.target.dataset.section);
    const index = Number(event.target.dataset.index);
    if (!Number.isInteger(section) || !Number.isInteger(index)) return;
    event.preventDefault();
    const next = moveInGrid(sizes, columns, { section, index }, event.key);
    if (next) focusAt(next);
  };

  const cell = (section: number, index: number) => ({
    "data-section": section,
    "data-index": index,
    tabIndex: section === active.section && index === active.index ? 0 : -1,
    onFocus: () => setActive({ section, index }),
    ref: (element: HTMLButtonElement | null) => {
      const key = `${section}:${index}`;
      if (element) cells.current.set(key, element);
      else cells.current.delete(key);
    },
  });

  return { cell, onKeyDown };
}
