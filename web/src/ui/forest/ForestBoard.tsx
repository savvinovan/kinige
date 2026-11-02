import { useEffect, useId, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import type { ForestDay } from "../../api/generated/ForestDay";
import type { ForestYear } from "../../api/generated/ForestYear";
import type { Season } from "../../api/generated/Season";
import { useLanguage } from "../../i18n/I18n";
import { buildBoard, lastSelectable, neighbourDay, rowDates, seasonCells } from "./board";
import { DayCard } from "./DayCard";
import { ForestEngine } from "./engine";
import { dayAnnouncement, monthNames, weekdayLetters } from "./forestText";
import type { Pt } from "./iso";
import "./forest.css";

/** A request from the header buttons: fit the whole year or bring a season into view. A new `nonce` is a new request. */
export interface ZoomRequest {
  target: "year" | Season;
  nonce: number;
}

interface ForestBoardProps {
  table: ForestYear;
  /** The day at the top of the feed: outlined on the board. */
  selected: string | null;
  zoom: ZoomRequest | null;
  /** A click on a tree, or Enter on the keyboard selection: open the day in the feed. */
  onOpenDay: (date: string) => void;
  /** The engine once it exists (and `null` when it is gone): the visual click-through test asks it where each day is. */
  onEngine?: (engine: ForestEngine | null) => void;
}

interface Hover {
  day: ForestDay;
  at: Pt;
  /** The size of the canvas when the pointer moved, to put the card on the side that has room. */
  size: { w: number; h: number };
}

/** Distance between the pointer and its card. */
const CARD_GAP = 16;

const STEPS: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };

/**
 * The isometric board of a year on a canvas (ADR-0013). The canvas is one focusable element: Tab into it, then left and right
 * for the previous and next day, up and down for the previous and next week, Enter to open the day in the feed, Escape to clear
 * the selection. The keyboard selection is announced in a live region ("October 16, 5 notes, a large tree"). The pointer
 * selects nothing by itself: hover shows the card, a click opens the day, a drag pans, wheel and pinch zoom.
 */
export function ForestBoard({ table, selected, zoom, onOpenDay, onEngine }: ForestBoardProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const hintId = useId();
  const canvas = useRef<HTMLCanvasElement>(null);
  const engine = useRef<ForestEngine | null>(null);
  const board = useMemo(() => buildBoard(table), [table]);
  const labels = useMemo(
    () => ({ months: monthNames(language), monthsNarrow: monthNames(language, "narrow"), weekdays: weekdayLetters(rowDates(board), language) }),
    [board, language],
  );
  const [hover, setHover] = useState<Hover | null>(null);
  // The keyboard selection belongs to the feed day it was made at: when the feed moves to another day it is dropped.
  const [keyboard, setKeyboard] = useState<{ anchor: string | null; date: string } | null>(null);
  const cursor = keyboard && keyboard.anchor === selected ? keyboard.date : null;

  // The first selectable day to start from when the keyboard has nothing yet.
  const start = selected !== null && board.byDate.get(selected)?.interactive ? selected : lastSelectable(board);
  const shown = cursor ?? (selected !== null && board.byDate.get(selected)?.interactive ? selected : null);
  const shownDay = shown !== null ? (board.byDate.get(shown)?.day ?? null) : null;

  // The latest values for the engine's callbacks, which outlive a render.
  const latest = useRef({ board, onOpenDay, onEngine });
  useEffect(() => {
    latest.current = { board, onOpenDay, onEngine };
  });

  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    const created = new ForestEngine(element, {
      onHover: (date, at) => {
        const day = date !== null ? latest.current.board.byDate.get(date)?.day : null;
        setHover(day && at ? { day, at, size: created.size } : null);
      },
      onActivate: (date) => latest.current.onOpenDay(date),
    });
    engine.current = created;
    latest.current.onEngine?.(created);
    return () => {
      latest.current.onEngine?.(null);
      created.destroy();
      engine.current = null;
    };
  }, []);

  useEffect(() => {
    engine.current?.setBoard(board, labels);
  }, [board, labels]);

  useEffect(() => {
    engine.current?.setSelected(shown, cursor !== null);
  }, [shown, cursor, board]);

  // One request, one move: the effect follows the request's number, not the identity of the object.
  const zoomTarget = zoom?.target;
  const zoomNonce = zoom?.nonce;
  useEffect(() => {
    if (zoomNonce === undefined || zoomTarget === undefined) return;
    if (zoomTarget === "year") engine.current?.fitYear();
    else engine.current?.fitCells(seasonCells(latest.current.board, zoomTarget));
  }, [zoomTarget, zoomNonce]);

  const onKeyDown = (event: KeyboardEvent<HTMLCanvasElement>) => {
    const step = STEPS[event.key];
    if (step !== undefined) {
      event.preventDefault();
      const from = cursor ?? start;
      if (from === null) return;
      setKeyboard({ anchor: selected, date: neighbourDay(board, from, step) ?? from });
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      const date = cursor ?? start;
      if (date !== null) onOpenDay(date);
    } else if (event.key === "Escape" && cursor !== null) {
      event.preventDefault();
      setKeyboard(null);
    }
  };

  const announcement = cursor !== null && shownDay ? dayAnnouncement(t, language, shownDay) : "";
  const cardAt = hover ? placeCard(hover.at, hover.size) : undefined;

  return (
    <div className="forest-board">
      <canvas
        ref={canvas}
        className="forest-board__canvas"
        tabIndex={0}
        role="application"
        aria-label={t("forest.board", { year: table.year })}
        aria-describedby={hintId}
        onKeyDown={onKeyDown}
        onBlur={() => setKeyboard(null)}
        onPointerDown={() => canvas.current?.focus({ preventScroll: true })}
      />
      <p className="visually-hidden" id={hintId}>
        {t("forest.boardHint")}
      </p>
      <div className="visually-hidden" role="status" aria-live="polite">
        {announcement}
      </div>
      {hover ? <DayCard day={hover.day} style={cardAt} /> : shownDay ? <DayCard day={shownDay} pinned /> : null}
    </div>
  );
}

/** The card sits at the pointer, on whichever side has room. */
function placeCard(at: Pt, size: { w: number; h: number }): CSSProperties {
  const style: CSSProperties = {};
  if (at.x > size.w / 2) style.right = size.w - at.x + CARD_GAP;
  else style.left = at.x + CARD_GAP;
  if (at.y > size.h / 2) style.bottom = size.h - at.y + CARD_GAP;
  else style.top = at.y + CARD_GAP;
  return style;
}
