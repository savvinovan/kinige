import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import "./drag.css";

/** The target attribute: a chat row in the sidebar and an icon in the collapsed strip carry `data-chat-drop="<chat id>"`. */
export const DROP_ATTRIBUTE = "data-chat-drop";

/** Bubble elements that dragging does not start from: they have their own press behavior. */
const INTERACTIVE = 'button, a, input, textarea, select, [role="checkbox"], [role="button"], [contenteditable="true"]';

interface DragOptions {
  /** Whether dropping onto this chat is allowed (the entry's current chat is not). */
  canDrop: (chatId: string, messageId: string) => boolean;
  /** A drop onto a chat: moving the entry. A drop elsewhere and Esc call nothing. */
  onDrop: (messageId: string, chatId: string) => void;
}

interface Ghost {
  x: number;
  y: number;
  width: number;
  className: string;
  html: string;
}

interface Session {
  messageId: string;
  plate: HTMLElement;
  pointerId: number;
  started: boolean;
}

/**
 * Dragging an entry bubble onto a chat in the sidebar (pointer events, not HTML5 DnD: in WKWebView that draws a system
 * "ghost", gives no control over the target highlight and interferes with text selection).
 *
 * Start: pressing the primary mouse button (not on a button, link, checkbox, tag or in the edit field) and the pointer leaving the
 * bubble bounds. While the pointer is inside the bubble this is ordinary text selection; dragging beyond the bubble with the button held is
 * how one reaches the sidebar, so leaving the edge is the movement threshold.
 *
 * After that: a quiet "ghost" (a copy of the bubble) under the cursor, the chat under the pointer is highlighted (`overChatId`), the page does not
 * select text. Releasing on a chat calls `onDrop`; releasing elsewhere, Esc, or the window losing focus do nothing. The click that
 * follows is suppressed so that the entry does not open a menu or a link.
 */
export function useRecordDrag({ canDrop, onDrop }: DragOptions): {
  /** The chat the pointer is currently over (for highlighting), or `null`. */
  overChatId: string | null;
  dragging: boolean;
  /** The bubble press handler: `begin(id)(event, plate)`. */
  begin: (messageId: string) => (event: ReactPointerEvent<HTMLElement>, plate: HTMLElement) => void;
  /** The "ghost": render it once next to the rest of the window. */
  ghost: ReactNode;
} {
  const [overChatId, setOverChatId] = useState<string | null>(null);
  const [ghost, setGhost] = useState<Ghost | null>(null);
  const session = useRef<Session | null>(null);
  const cleanup = useRef<(() => void) | null>(null);
  const latest = useRef({ canDrop, onDrop });
  useEffect(() => {
    latest.current = { canDrop, onDrop };
  });

  const stop = useCallback(() => {
    cleanup.current?.();
    cleanup.current = null;
    session.current = null;
    document.body.classList.remove("is-record-dragging");
    setGhost(null);
    setOverChatId(null);
  }, []);

  useEffect(() => stop, [stop]);

  const begin = useCallback(
    (messageId: string) => (event: ReactPointerEvent<HTMLElement>, plate: HTMLElement) => {
      if (session.current || event.button !== 0 || event.pointerType === "touch") return;
      if (event.target instanceof Element && event.target.closest(INTERACTIVE)) return;
      const current: Session = {
        messageId,
        plate,
        pointerId: event.pointerId,
        started: false,
      };
      session.current = current;
      let over: string | null = null;

      const target = (x: number, y: number): string | null => {
        const element = document.elementFromPoint(x, y)?.closest(`[${DROP_ATTRIBUTE}]`);
        const id = element?.getAttribute(DROP_ATTRIBUTE) ?? null;
        return id !== null && latest.current.canDrop(id, messageId) ? id : null;
      };

      const onMove = (move: PointerEvent) => {
        if (move.pointerId !== current.pointerId) return;
        if (!current.started) {
          const box = current.plate.getBoundingClientRect();
          const inside = move.clientX >= box.left && move.clientX <= box.right && move.clientY >= box.top && move.clientY <= box.bottom;
          if (inside) return;
          current.started = true;
          window.getSelection()?.removeAllRanges();
          document.body.classList.add("is-record-dragging");
        }
        const box = current.plate.getBoundingClientRect();
        setGhost({
          // The "ghost" stands to the right of and below the pointer (the offset is a token in CSS): the chat under the pointer stays visible.
          x: move.clientX,
          y: move.clientY,
          width: box.width,
          className: current.plate.className,
          html: current.plate.innerHTML,
        });
        const next = target(move.clientX, move.clientY);
        if (next !== over) {
          over = next;
          setOverChatId(next);
        }
      };

      const onUp = (up: PointerEvent) => {
        if (up.pointerId !== current.pointerId) return;
        const drop = current.started ? target(up.clientX, up.clientY) : null;
        const wasDragging = current.started;
        stop();
        if (wasDragging) {
          // A click after a drop must not open a menu, link or tag under the cursor.
          const swallow = (click: Event) => {
            click.stopPropagation();
            click.preventDefault();
          };
          window.addEventListener("click", swallow, { capture: true, once: true });
          setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
        }
        if (drop) latest.current.onDrop(messageId, drop);
      };

      const onKey = (key: KeyboardEvent) => {
        if (key.key !== "Escape") return;
        // Esc belongs to the drag: the window and the filters do not get it.
        key.stopPropagation();
        key.preventDefault();
        stop();
      };
      const onCancel = () => stop();
      const onSelectStart = (select: Event) => {
        if (current.started) select.preventDefault();
      };

      document.addEventListener("pointermove", onMove);
      document.addEventListener("pointerup", onUp);
      document.addEventListener("pointercancel", onCancel);
      document.addEventListener("keydown", onKey, true);
      document.addEventListener("selectstart", onSelectStart);
      window.addEventListener("blur", onCancel);
      cleanup.current = () => {
        document.removeEventListener("pointermove", onMove);
        document.removeEventListener("pointerup", onUp);
        document.removeEventListener("pointercancel", onCancel);
        document.removeEventListener("keydown", onKey, true);
        document.removeEventListener("selectstart", onSelectStart);
        window.removeEventListener("blur", onCancel);
      };
    },
    [stop],
  );

  return {
    overChatId,
    dragging: ghost !== null,
    begin,
    ghost: ghost
      ? createPortal(
          <div className="record-ghost" aria-hidden="true" style={{ left: ghost.x, top: ghost.y, width: ghost.width }}>
            <article className={ghost.className} dangerouslySetInnerHTML={{ __html: ghost.html }} />
          </div>,
          document.body,
        )
      : null,
  };
}
