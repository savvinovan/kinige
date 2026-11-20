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

/** The custom property the row's `transform` reads (message.css): how far the row has been moved to stay under the pointer. */
const SHIFT = "--actions-shift";

/**
 * The entry the pointer or the keyboard is on (slice 5a). A put or a removal adds or takes away a row of chips; the feed grows the bubble upwards, so a row
 * anchored to the bubble's top would move and the next click would land on the text. While the entry is held, every change of the wrapper's top is
 * handed back as a shift of the row, in the same frame (a layout effect or a resize observer, before paint), so the pressed button stays where the
 * pointer is, whether the feed is at its end or scrolled. `top` is the wrapper's top as of the last look.
 */
interface Held {
  top: number;
  shift: number;
}

/** Hands the movement of the wrapper since the last look back to the row. Only the row beside the top edge ("inside") moves with it. */
function settle(element: HTMLElement, held: Held | null, inside: boolean): void {
  if (!held) return;
  const top = element.getBoundingClientRect().top;
  held.shift = inside ? held.shift + held.top - top : 0;
  held.top = top;
  element.querySelector<HTMLElement>(".message-actions")?.style.setProperty(SHIFT, `${held.shift}px`);
}

/**
 * Where the action row stands and when it is visible.
 *
 * - `inside`: there is not enough room to the right of the bubble (bubble + row wider than the column) - the row moves to the top border of the bubble
 *   at its right edge (slightly above the bubble). The width of the row's outer layer does not depend on the mode, so there is no flicker at the threshold.
 * - `flip`: there is no room above the bubble (the first message at the top edge of the feed) - the row is pressed inside the bubble instead of being clipped.
 * - `shown`: the hover zone is the whole feed row (`.feed__item` across the column), not just the bubble; outside the feed, the wrapper itself.
 *   After the mouse leaves, the row stays for another `motion.delay.hide-actions` (about 180 ms): you can reach the icon diagonally.
 * - while the entry is held (pointer in it, or focus inside it) the row keeps its place on screen when the bubble changes height (`Held`), and `flip` is
 *   measured when the entry is first held (pointer or focus) and on scrolling, not when the bubble changes height, so a row shown by the keyboard for
 *   an entry that came into view later is not on its text, and the row does not jump when a put makes the bubble taller near the feed's top edge.
 */
export function useActionsPlacement(deps: unknown) {
  const wrap = useRef<HTMLDivElement>(null);
  const [inside, setInside] = useState(false);
  const [flip, setFlip] = useState(false);
  const [shown, setShown] = useState(false);
  const held = useRef<Held | null>(null);

  useLayoutEffect(() => {
    const element = wrap.current;
    const column = element?.parentElement;
    const actions = element?.querySelector<HTMLElement>(".message-actions");
    if (!element || !column || !actions) return;
    const measure = () => {
      const beside = element.offsetWidth + actions.offsetWidth > column.clientWidth;
      setInside(beside);
      // A held entry keeps its `flip`: the bubble grew upwards, the row is anchored where the pointer found it, and a new look at the room above would
      // move the row under the pointer (found in the real window: the row jumped by the height of the chips row near the feed's top edge).
      if (!held.current) setFlip(noRoomAbove(element));
      settle(element, held.current, beside);
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
    const scroller = element.closest<HTMLElement>(".feed-scroll");
    let timer: ReturnType<typeof setTimeout> | undefined;
    let pointer = false;
    let focus = false;

    // Scrolling starts the row's placement afresh: the shift goes, the reference point moves and `flip` is measured again.
    const scrolled = () => {
      if (held.current) {
        held.current = { top: element.getBoundingClientRect().top, shift: 0 };
        element.querySelector<HTMLElement>(".message-actions")?.style.removeProperty(SHIFT);
      }
      setFlip(noRoomAbove(element));
    };
    const hold = () => {
      if (held.current) return;
      setFlip(noRoomAbove(element));
      held.current = { top: element.getBoundingClientRect().top, shift: 0 };
      scroller?.addEventListener("scroll", scrolled, { passive: true });
    };
    const drop = () => {
      if (pointer || focus || !held.current) return;
      held.current = null;
      scroller?.removeEventListener("scroll", scrolled);
      element.querySelector<HTMLElement>(".message-actions")?.style.removeProperty(SHIFT);
    };

    const enter = () => {
      clearTimeout(timer);
      pointer = true;
      hold();
      setShown(true);
    };
    const leave = () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        pointer = false;
        setShown(false);
        drop();
      }, hideDelayMs());
    };
    const focusIn = () => {
      focus = true;
      hold();
    };
    const focusOut = (event: FocusEvent) => {
      if (event.relatedTarget instanceof Node && element.contains(event.relatedTarget)) return;
      focus = false;
      drop();
    };

    zone.addEventListener("pointerenter", enter);
    zone.addEventListener("pointerleave", leave);
    element.addEventListener("focusin", focusIn);
    element.addEventListener("focusout", focusOut);
    return () => {
      clearTimeout(timer);
      scroller?.removeEventListener("scroll", scrolled);
      zone.removeEventListener("pointerenter", enter);
      zone.removeEventListener("pointerleave", leave);
      element.removeEventListener("focusin", focusIn);
      element.removeEventListener("focusout", focusOut);
    };
  }, []);

  return { wrap, inside, flip, shown };
}
