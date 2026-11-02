import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { KinigeApi } from "../../api/KinigeApi";
import type { ForestMode } from "../../api/generated/ForestMode";
import type { ForestView } from "../../api/generated/ForestView";
import type { ForestYear } from "../../api/generated/ForestYear";
import { detectFirstWeekday } from "../forest/firstWeekday";
import { ForestPane } from "../forest/ForestPane";

interface UseForestPaneOptions {
  api: KinigeApi;
  /** The state saved from the last run (`Settings`), read before the first frame. */
  initial: { view: ForestView; mode: ForestMode };
  /** The open chat; `null` in the trash. A click prefers it when it has entries that day. */
  chatId: string | null;
  /** Any change here reloads the table: local entry changes, the "entries changed" event, a new journal day. */
  refreshKey: string;
  /** Opens the feed on the first entry of the target chat that day (switches the chat if needed). */
  openEntry: (chatId: string, messageId: string) => void;
}

/**
 * The forest above the feed (slice 6): its view state (hidden, strip, expanded; board or calendar), the year table from Rust,
 * the day the feed shows at its top (the outlined cell; the board shows the year of that day), and the click that opens a day.
 * The first weekday comes from the system locale once; the table is not requested while the forest is hidden.
 */
export function useForestPane({ api, initial, chatId, refreshKey, openEntry }: UseForestPaneOptions) {
  const [state, setState] = useState(initial);
  const [year, setYear] = useState(() => new Date().getFullYear());
  const [table, setTable] = useState<ForestYear | null>(null);
  const [failed, setFailed] = useState(false);
  const [feedDay, setFeedDay] = useState<string | null>(null);
  const feedDayRef = useRef<string | null>(null);
  const weekday = useMemo(() => detectFirstWeekday(), []);
  const sourceSent = useRef(false);
  const hidden = state.view === "hidden";

  // The table of the shown year. A newer request wins; the table on screen stays until the next one arrives.
  useEffect(() => {
    if (hidden) return;
    let live = true;
    const source = sourceSent.current ? undefined : weekday.source;
    sourceSent.current = true;
    api
      .getForestYear(year, weekday.value, source)
      .then((next) => {
        if (!live) return;
        setTable(next);
        setFailed(false);
      })
      .catch(() => live && setFailed(true));
    return () => {
      live = false;
    };
  }, [api, year, weekday, hidden, refreshKey]);

  // The feed scrolls: its top day is the outlined one, and when the day moves into another year the board follows.
  const onTopDay = useCallback((date: string) => {
    const previous = feedDayRef.current;
    feedDayRef.current = date;
    setFeedDay(date);
    if (previous === null || previous.slice(0, 4) !== date.slice(0, 4)) setYear(Number(date.slice(0, 4)));
  }, []);

  const change = useCallback(
    (next: { view: ForestView; mode: ForestMode }) => {
      setState(next);
      api.setForestView(next.view, next.mode).catch(() => undefined);
    },
    [api],
  );

  const openDay = useCallback(
    (date: string) => {
      api
        .getForestDayTarget(date, chatId)
        .then((target) => {
          if (!target) return;
          // The outline stays on the clicked day until the person scrolls the feed (the feed centres the entry, so the day at
          // its top is an earlier one).
          feedDayRef.current = date;
          setFeedDay(date);
          openEntry(target.chatId, target.messageId);
        })
        .catch(() => undefined);
    },
    [api, chatId, openEntry],
  );

  const pane: ReactNode =
    state.view === "hidden" ? null : (
      <ForestPane
        view={state.view}
        mode={state.mode}
        table={table}
        year={year}
        minYear={Math.min(table?.minYear ?? year, year)}
        maxYear={Math.max(Number(table?.today.slice(0, 4) ?? year), year)}
        selected={feedDay}
        failed={failed}
        onOpenDay={openDay}
        onYearChange={setYear}
        onView={(view) => change({ ...state, view })}
        onMode={(mode) => change({ ...state, mode })}
        onHide={() => change({ ...state, view: "hidden" })}
      />
    );

  return {
    pane,
    hidden,
    onTopDay,
    /** "Show forest" brings the strip back; "Hide forest" hides it. */
    toggleHidden: () => change({ ...state, view: hidden ? "strip" : "hidden" }),
  };
}
