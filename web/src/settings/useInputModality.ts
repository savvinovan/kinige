import { useEffect } from "react";

/**
 * Откуда пришёл последний ввод: `keyboard` после нажатия клавиши, `pointer` после мыши или касания. Значение — атрибут
 * `data-input` на `<html>`; кольцо фокуса в окне настроек показывается только в режиме `keyboard` (`ui/prefs/prefs.css`).
 * Пока ничего не нажато (окно только открылось, фокус поставлен системой или программно), колец нет: WKWebView фокусирует
 * первый пункт сам, и он выглядел как «рамка» вокруг выбранного раздела.
 */
export function useInputModality(): void {
  useEffect(() => {
    const root = document.documentElement;
    root.dataset.input = "pointer";
    const keyboard = (event: KeyboardEvent) => {
      // Сами по себе модификаторы (⌘, ⌥) режим не меняют: ими открывают окно и переключают приложения.
      if (event.key === "Meta" || event.key === "Alt" || event.key === "Control" || event.key === "Shift") return;
      root.dataset.input = "keyboard";
    };
    const pointer = () => {
      root.dataset.input = "pointer";
    };
    document.addEventListener("keydown", keyboard, true);
    document.addEventListener("pointerdown", pointer, true);
    document.addEventListener("mousedown", pointer, true);
    return () => {
      document.removeEventListener("keydown", keyboard, true);
      document.removeEventListener("pointerdown", pointer, true);
      document.removeEventListener("mousedown", pointer, true);
      delete root.dataset.input;
    };
  }, []);
}
