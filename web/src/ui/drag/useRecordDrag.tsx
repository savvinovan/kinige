import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import "./drag.css";

/** Атрибут цели: у строки чата в сайдбаре и у значка в свёрнутой полосе стоит `data-chat-drop="<id чата>"`. */
export const DROP_ATTRIBUTE = "data-chat-drop";

/** Элементы плашки, с которых перетаскивание не начинается: у них своё поведение при нажатии. */
const INTERACTIVE = 'button, a, input, textarea, select, [role="checkbox"], [role="button"], [contenteditable="true"]';

interface DragOptions {
  /** Можно ли бросить на этот чат (текущий чат записи — нельзя). */
  canDrop: (chatId: string, messageId: string) => boolean;
  /** Бросок на чат: перенос записи. Бросок мимо и Esc ничего не вызывают. */
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
  /** Где в плашке взялись за неё: «призрак» держится за ту же точку. */
  grabX: number;
  grabY: number;
  pointerId: number;
  started: boolean;
}

/**
 * Перетаскивание плашки записи на чат в сайдбаре (события указателя, не HTML5 DnD: тот в WKWebView рисует системный
 * «призрак», не даёт управлять подсветкой цели и вмешивается в выделение текста).
 *
 * Начало: нажатие основной кнопкой мыши (не на кнопке, ссылке, флажке, теге и не в поле правки) и выход указателя за границы
 * плашки. Пока указатель внутри плашки, это обычное выделение текста; тащить за пределы плашки при нажатой кнопке и нужно,
 * чтобы дойти до сайдбара, поэтому выход за край и есть порог смещения.
 *
 * Дальше: под курсором тихий «призрак» (копия плашки), чат под указателем подсвечивается (`overChatId`), страница не
 * выделяет текст. Отпускание на чате — `onDrop`; мимо, Esc, потеря фокуса окном — ничего. Следом идущий щелчок
 * подавляется, чтобы запись не открывала меню или ссылку.
 */
export function useRecordDrag({ canDrop, onDrop }: DragOptions): {
  /** Чат, над которым сейчас указатель (для подсветки), или `null`. */
  overChatId: string | null;
  dragging: boolean;
  /** Обработчик нажатия плашки: `begin(id)(event, plate)`. */
  begin: (messageId: string) => (event: ReactPointerEvent<HTMLElement>, plate: HTMLElement) => void;
  /** «Призрак»: отрисовать один раз рядом с остальным окном. */
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
      const rect = plate.getBoundingClientRect();
      const current: Session = {
        messageId,
        plate,
        grabX: event.clientX - rect.left,
        grabY: event.clientY - rect.top,
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
          x: move.clientX - current.grabX,
          y: move.clientY - current.grabY,
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
          // Щелчок после броска не должен открыть меню, ссылку или тег под курсором.
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
        // Esc принадлежит перетаскиванию: окно и фильтры его не получают.
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
