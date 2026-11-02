import { useCallback, useRef, useState, type KeyboardEvent } from "react";
import { type Board, lastSelectable, neighbourDay } from "./board";

/** Which arrow moves by how many days (negative: back). The strip and the calendar differ only in this map. */
export type DayKeys = Record<string, number>;

/**
 * Roving tabindex over the days of a year for the strip and the calendar: one day is in the Tab order (the one the feed shows,
 * or the last day that can be selected), arrows move focus along the calendar order, Enter and Space open the day. Every
 * day's element carries `data-date`, so focus moves without waiting for a render.
 */
export function useRovingDay(board: Board, selected: string | null, keys: DayKeys, onOpenDay: (date: string) => void) {
  const root = useRef<HTMLElement | null>(null);
  const rootRef = useCallback((element: HTMLElement | null) => {
    root.current = element;
  }, []);
  const [rover, setRover] = useState<string | null>(null);
  const usable = (date: string | null) => (date !== null && board.byDate.get(date)?.interactive ? date : null);
  const tabbable = usable(rover) ?? usable(selected) ?? lastSelectable(board);

  const focusDay = (date: string) => {
    setRover(date);
    root.current?.querySelector<HTMLElement>(`[data-date="${date}"]`)?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    const date = (event.target as HTMLElement).closest<HTMLElement>("[data-date]")?.dataset.date;
    if (!date) return;
    const step = keys[event.key];
    if (step !== undefined) {
      event.preventDefault();
      const next = neighbourDay(board, date, step);
      if (next) focusDay(next);
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (board.byDate.get(date)?.interactive) onOpenDay(date);
    }
  };

  return { rootRef, tabbable, onKeyDown, onDayFocus: setRover };
}
