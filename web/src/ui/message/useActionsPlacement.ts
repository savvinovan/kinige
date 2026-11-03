import { useEffect, useLayoutEffect, useRef, useState } from "react";

/** Fallback hide delay (ms) if the `motion.delay.hide-actions` token is unavailable (tests without CSS). */
const FALLBACK_HIDE_DELAY_MS = 180;

/** Hide delay from the `--motion-delay-hide-actions` token ("0.18s" after Style Dictionary). Reduce Motion does not change it. */
export function hideDelayMs(): number {
  const raw = getComputedStyle(document.documentElement).getPropertyValue("--motion-delay-hide-actions").trim();
  const value = Number.parseFloat(raw);
  if (Number.isNaN(value)) return FALLBACK_HIDE_DELAY_MS;
  return raw.endsWith("ms") ? value : value * 1000;
}

/**
 * There is no room above the bubble for the row (the first message at the top edge of the feed). The row above the bubble takes its height minus
 * the part that goes inside the bubble (its top padding). Outside the feed (no `.feed-scroll`) there is no limit at the top.
 */
function noRoomAbove(element: HTMLElement): boolean {
  const scroller = element.closest<HTMLElement>(".feed-scroll");
  const actions = element.querySelector<HTMLElement>(".message-actions");
  const plate = element.querySelector<HTMLElement>(".message");
  if (!scroller || !actions || !plate) return false;
  const above = actions.offsetHeight - (Number.parseFloat(getComputedStyle(plate).paddingTop) || 0);
  return element.getBoundingClientRect().top - scroller.getBoundingClientRect().top < above;
}

/**
 * Where the action row stands and when it is visible.
 *
 * - `inside`: there is not enough room to the right of the bubble (bubble + row wider than the column) - the row moves to the top border of the bubble
 *   at its right edge (slightly above the bubble). The width of the row's outer layer does not depend on the mode, so there is no flicker at the threshold.
 * - `flip`: there is no room above the bubble (the first message at the top edge of the feed) - the row is pressed inside the bubble instead of being clipped.
 * - `shown`: the hover zone is the whole feed row (`.feed__item` across the column), not just the bubble; outside the feed, the wrapper itself.
 *   After the mouse leaves, the row stays for another `motion.delay.hide-actions` (about 180 ms): you can reach the icon diagonally.
 */
export function useActionsPlacement(deps: unknown) {
  const wrap = useRef<HTMLDivElement>(null);
  const [inside, setInside] = useState(false);
  const [flip, setFlip] = useState(false);
  const [shown, setShown] = useState(false);

  useLayoutEffect(() => {
    const element = wrap.current;
    const column = element?.parentElement;
    const actions = element?.querySelector<HTMLElement>(".message-actions");
    if (!element || !column || !actions) return;
    const measure = () => {
      setInside(element.offsetWidth + actions.offsetWidth > column.clientWidth);
      setFlip(noRoomAbove(element));
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(column);
    observer.observe(element);
    return () => observer.disconnect();
  }, [deps]);

  useEffect(() => {
    const element = wrap.current;
    if (!element) return;
    const zone = element.closest<HTMLElement>(".feed__item") ?? element;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const enter = () => {
      clearTimeout(timer);
      setFlip(noRoomAbove(element));
      setShown(true);
    };
    const leave = () => {
      clearTimeout(timer);
      timer = setTimeout(() => setShown(false), hideDelayMs());
    };

    zone.addEventListener("pointerenter", enter);
    zone.addEventListener("pointerleave", leave);
    return () => {
      clearTimeout(timer);
      zone.removeEventListener("pointerenter", enter);
      zone.removeEventListener("pointerleave", leave);
    };
  }, []);

  return { wrap, inside, flip, shown };
}
