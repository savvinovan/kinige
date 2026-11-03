import { useEffect, useRef } from "react";
import type { KinigeApi } from "../../api/KinigeApi";
import type { FocusTarget } from "./useFeed";

interface UseOpenMessageOptions {
  api: KinigeApi;
  /** Open a chat (the previous chat's draft goes to Rust before switching). */
  select: (chatId: string | null) => Promise<void>;
  /** The feed opens the chat at an entry (`useFeed`). */
  onFocus: (target: FocusTarget) => void;
}

/**
 * The "Settings" window asks to open an entry from the access log (the `open-message` event): Rust has already checked
 * that the entry exists. The main window opens its chat and puts the feed on the entry.
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
