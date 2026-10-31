import { useEffect, useRef, useState } from "react";

/** Сколько «Скопировано» остаётся в подписи кнопки, прежде чем вернуться к «Копировать». */
const COPIED_MS = 2000;

export type CopyState = "idle" | "copied" | "failed";

/**
 * Выделенный текст внутри `root` или `null`, если выделения нет или оно выходит за пределы `root`. Выделение берётся из
 * отрисованного текста (без Markdown-разметки): так копируется ровно то, что человек выделил мышью.
 */
export function selectedTextIn(root: Element | null): string | null {
  const selection = window.getSelection();
  if (!root || !selection || selection.isCollapsed || selection.rangeCount === 0) return null;
  if (!root.contains(selection.anchorNode) || !root.contains(selection.focusNode)) return null;
  const text = selection.toString();
  return text === "" ? null : text;
}

/** Запись в буфер обмена и состояние подписи кнопки: «Скопировано» или честное «Не удалось скопировать». */
export function useCopy(resetAfterMs: number = COPIED_MS) {
  const [state, setState] = useState<CopyState>("idle");
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timer.current), []);

  const copy = async (text: string) => {
    let next: CopyState = "copied";
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // Буфер обмена недоступен (нет разрешения, не защищённый контекст): честно говорим об этом, а не делаем вид.
      next = "failed";
    }
    setState(next);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setState("idle"), resetAfterMs);
  };

  return { state, copy };
}
