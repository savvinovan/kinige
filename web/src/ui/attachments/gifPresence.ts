import { useSyncExternalStore } from "react";

// What decides whether a GIF may play right now, apart from the GIF itself: whether its cell is on screen, whether the window is visible, and whether
// Reduce Motion is on. Each is one shared source, so a feed of fifty GIFs has one observer and one listener per signal, not fifty.

/** The share of a cell that must be on screen for its GIF to play. */
export const GIF_VISIBLE_SHARE = 0.5;

type Listener = (visible: boolean) => void;
const listeners = new Map<Element, Listener>();
let observer: IntersectionObserver | null = null;

/**
 * Tells `listener` whether at least half of `element` is on screen, through the one `IntersectionObserver` of all GIF cells (it exists while any
 * cell is watched). Returns the function that stops watching. Without `IntersectionObserver` nothing is ever reported, so nothing plays.
 */
export function observeVisibility(element: Element, listener: Listener): () => void {
  if (typeof IntersectionObserver === "undefined") return () => undefined;
  observer ??= new IntersectionObserver(
    (entries) => {
      for (const entry of entries) listeners.get(entry.target)?.(entry.isIntersecting && entry.intersectionRatio >= GIF_VISIBLE_SHARE);
    },
    { threshold: [0, GIF_VISIBLE_SHARE] },
  );
  listeners.set(element, listener);
  observer.observe(element);
  return () => {
    listeners.delete(element);
    observer?.unobserve(element);
    if (listeners.size === 0) {
      observer?.disconnect();
      observer = null;
    }
  };
}

function subscribeVisibility(notify: () => void) {
  document.addEventListener("visibilitychange", notify);
  return () => document.removeEventListener("visibilitychange", notify);
}

/** Whether the window is visible (not hidden to the menu bar, minimised or on another desktop): a hidden window plays nothing. */
export function useWindowVisible(): boolean {
  return useSyncExternalStore(subscribeVisibility, () => document.visibilityState !== "hidden", () => true);
}

const REDUCE = "(prefers-reduced-motion: reduce)";
const ATTRIBUTE = "data-reduce-motion";

function subscribeMotion(notify: () => void) {
  const media = typeof matchMedia === "function" ? matchMedia(REDUCE) : null;
  // The `change` event is how the OS setting is followed while the window is open; the attribute is the Storybook switch and the same setting's twin.
  media?.addEventListener("change", notify);
  const watch = typeof MutationObserver === "function" ? new MutationObserver(notify) : null;
  watch?.observe(document.documentElement, { attributes: true, attributeFilter: [ATTRIBUTE] });
  return () => {
    media?.removeEventListener("change", notify);
    watch?.disconnect();
  };
}

function reducedNow(): boolean {
  const media = typeof matchMedia === "function" && matchMedia(REDUCE).matches;
  return media || document.documentElement.hasAttribute(ATTRIBUTE);
}

/** Whether Reduce Motion is on, followed live: the OS setting (`prefers-reduced-motion`) or the `data-reduce-motion` attribute of the window. */
export function useReducedMotion(): boolean {
  return useSyncExternalStore(subscribeMotion, reducedNow, () => false);
}
