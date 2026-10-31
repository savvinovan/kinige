import { useEffect, useRef } from "react";
import type { KinigeApi } from "../../api/KinigeApi";
import type { FocusTarget } from "./useFeed";

interface UseOpenMessageOptions {
  api: KinigeApi;
  /** Открыть чат (черновик прежнего уходит в Rust до переключения). */
  select: (chatId: string | null) => Promise<void>;
  /** Лента открывает чат на записи (`useFeed`). */
  onFocus: (target: FocusTarget) => void;
}

/**
 * Окно «Настройки» просит открыть запись из журнала обращений (событие `open-message`): Rust уже проверил, что запись есть.
 * Главное окно открывает её чат и ставит ленту на запись.
 */
export function useOpenMessage({ api, select, onFocus }: UseOpenMessageOptions): void {
  const counter = useRef(0);
  const latest = useRef({ select, onFocus });
  useEffect(() => {
    latest.current = { select, onFocus };
  });

  useEffect(() => {
    let closed = false;
    let off: (() => void) | undefined;
    api
      .onOpenMessage(({ chatId, messageId }) => {
        void latest.current.select(chatId).then(() => {
          counter.current += 1;
          latest.current.onFocus({ chatId, messageId, terms: [], nonce: counter.current });
        });
      })
      .then((unlisten) => {
        if (closed) unlisten();
        else off = unlisten;
      })
      .catch(() => undefined);
    return () => {
      closed = true;
      off?.();
    };
  }, [api]);
}
