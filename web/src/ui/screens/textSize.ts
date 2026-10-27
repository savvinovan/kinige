import type { TextSize } from "../../api/generated/TextSize";

/** Размер текста окна: атрибут на `<html>` читают токены (`data-text-scale`, ADR-0004). */
export function applyTextSize(size: TextSize): void {
  const root = document.documentElement;
  if (size === "large") root.setAttribute("data-text-scale", "large");
  else root.removeAttribute("data-text-scale");
}
