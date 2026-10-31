import { useEffect } from "react";

/**
 * Откуда пришёл последний ввод: `keyboard` после нажатия клавиши, `pointer` после мыши или касания. Значение — атрибут
 * `data-input` на `<html>`; хук вызывают корни всех окон (главное, захват, настройки). Кольцо фокуса (`:focus-visible`) в режиме
 * `pointer` не рисуется (общее правило в `ui/base.css`), в режиме `keyboard` рисуется. Пока ничего не нажато (окно только
 * открылось, фокус поставлен системой или программно) режим `pointer`: WKWebView фокусирует первый пункт сам, а меню после
 * закрытия возвращает фокус на кнопку-триггер (это нужно для доступности), и оба раза кольцо выглядело бы «рамкой» без причины.
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
