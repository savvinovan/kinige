import { useLayoutEffect, useRef, type RefObject } from "react";

/**
 * Bubble width before editing: while the bubble is normal, its width is remembered (and updated on resize); on entering
 * edit mode it is pinned by the `--message-edit-width` variable, so the bubble changes only its height.
 * (Moved out of `Message.tsx` in KINIG-138 to keep that file under the size cap.)
 */
export function useEditWidth(plate: RefObject<HTMLElement | null>, mode: "view" | "editing" | "deleted", html: string) {
  const viewWidth = useRef(0);
  useLayoutEffect(() => {
    const element = plate.current;
    if (!element || mode === "editing") return;
    viewWidth.current = element.offsetWidth;
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      viewWidth.current = element.offsetWidth;
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [plate, mode, html]);
  useLayoutEffect(() => {
    const element = plate.current;
    if (!element || mode !== "editing") return;
    if (viewWidth.current > 0) element.style.setProperty("--message-edit-width", `${viewWidth.current}px`);
    return () => {
      element.style.removeProperty("--message-edit-width");
    };
  }, [plate, mode]);
}
