// Which labels of the board's edges are drawn (slice 6). The decision is made in screen pixels against the size of the label
// font, which comes from the type tokens, so large text and narrow windows give fewer, not overlapping, labels instead of none.

/** The room a label wants around it, in font sizes: weekday rows closer than this apart pile up. */
const ROW_ROOM = 1.3;
/** Space kept between two month labels, in pixels. */
const MONTH_GAP = 4;

/** Pixels of the font size in a canvas `font` string (`"400 13px Inter"`), or the fallback when there is none. */
export function fontPx(font: string, fallback = 13): number {
  const found = /(\d+(?:\.\d+)?)px/.exec(font);
  const size = found ? Number(found[1]) : NaN;
  return Number.isFinite(size) && size > 0 ? size : fallback;
}

/** Every weekday letter (1), every other one (2) or none (0), by the distance between two rows on the screen. */
export function weekdayStep(rowPitch: number, font: number): 0 | 1 | 2 {
  if (rowPitch >= font * ROW_ROOM) return 1;
  return rowPitch * 2 >= font * ROW_ROOM ? 2 : 0;
}

export interface MonthSpot {
  /** Screen x of the label's centre. */
  x: number;
  full: string;
  narrow: string;
}

/**
 * The month labels to draw, left to right: the full names when all of them fit, else the one-letter ones, and in either form a
 * label that would touch the one before it is left out.
 */
export function pickMonths<T extends MonthSpot>(spots: T[], measure: (text: string) => number): { spot: T; text: string }[] {
  const place = (text: (spot: T) => string) => {
    const picked: { spot: T; text: string; half: number }[] = [];
    let dropped = false;
    for (const spot of spots) {
      const label = text(spot);
      if (!label) continue;
      const width = measure(label);
      const half = Number.isFinite(width) ? width / 2 : 0;
      const last = picked[picked.length - 1];
      if (last && spot.x - last.spot.x < last.half + half + MONTH_GAP) dropped = true;
      else picked.push({ spot, text: label, half });
    }
    return { picked, dropped };
  };
  const full = place((spot) => spot.full);
  return (full.dropped ? place((spot) => spot.narrow) : full).picked;
}
