import type { TextSize } from "../../api/generated/TextSize";

/** Window text size: the tokens read the attribute on `<html>` (`data-text-scale`, ADR-0004). */
export function applyTextSize(size: TextSize): void {
  const root = document.documentElement;
  if (size === "large") root.setAttribute("data-text-scale", "large");
  else root.removeAttribute("data-text-scale");
}
