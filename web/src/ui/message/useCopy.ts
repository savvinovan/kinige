import { useEffect, useRef, useState } from "react";

/** How long "Copied" stays in the button label before it returns to "Copy". */
const COPIED_MS = 2000;

export type CopyState = "idle" | "copied" | "failed";

/**
 * The selected text inside `root`, or `null` if there is no selection or it goes beyond `root`. The selection is taken from
 * the rendered text (without Markdown markup): so exactly what the person selected with the mouse is copied.
 */
export function selectedTextIn(root: Element | null): string | null {
  const selection = window.getSelection();
  if (!root || !selection || selection.isCollapsed || selection.rangeCount === 0) return null;
  if (!root.contains(selection.anchorNode) || !root.contains(selection.focusNode)) return null;
  const text = selection.toString();
  return text === "" ? null : text;
}

/** Writing to the clipboard and the button label state: "Copied" or an honest "Could not copy". */
export function useCopy(resetAfterMs: number = COPIED_MS) {
  const [state, setState] = useState<CopyState>("idle");
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timer.current), []);

  const copy = async (text: string) => {
    let next: CopyState = "copied";
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // The clipboard is unavailable (no permission, insecure context): we say so honestly instead of pretending.
      next = "failed";
    }
    setState(next);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setState("idle"), resetAfterMs);
  };

  return { state, copy };
}
