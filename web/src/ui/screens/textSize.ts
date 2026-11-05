import type { TextSize } from "../../api/generated/TextSize";

/** The attribute on `<html>` the tokens read for the window text size (ADR-0004). */
export const TEXT_SCALE_ATTRIBUTE = "data-text-scale";

/** Window text size: the tokens read the attribute on `<html>` (`TEXT_SCALE_ATTRIBUTE`, ADR-0004). */
export function applyTextSize(size: TextSize): void {
  const root = document.documentElement;
  if (size === "large") root.setAttribute(TEXT_SCALE_ATTRIBUTE, "large");
  else root.removeAttribute(TEXT_SCALE_ATTRIBUTE);
}
