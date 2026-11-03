import { useEffect } from "react";

/**
 * Where the last input came from: `keyboard` after a key press, `pointer` after a mouse or touch. The value is the
 * `data-input` attribute on `<html>`; the hook is called by the roots of all windows (main, capture, settings). The focus ring (`:focus-visible`) is
 * not drawn in `pointer` mode (the shared rule in `ui/base.css`) and is drawn in `keyboard` mode. While nothing has been pressed (the window has just
 * opened, focus was set by the system or programmatically) the mode is `pointer`: WKWebView focuses the first item itself, and a menu after
 * closing returns focus to the trigger button (needed for accessibility), and both times the ring would look like a "frame" for no reason.
 */
export function useInputModality(): void {
  useEffect(() => {
    const root = document.documentElement;
    root.dataset.input = "pointer";
    const keyboard = (event: KeyboardEvent) => {
      // Modifiers on their own (⌘, ⌥) do not change the mode: they are used to open the window and switch apps.
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
