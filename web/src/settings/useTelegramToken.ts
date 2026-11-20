import { useEffect, useState } from "react";
import { type KinigeApi, errorCode } from "../api/KinigeApi";
import type { ErrorCode } from "../api/generated/ErrorCode";
import type { TelegramTokenState } from "../api/generated/TelegramTokenState";
import type { TokenControls } from "../ui/prefs/pages/StickersPage";

/**
 * The Telegram bot token of the Stickers page (slice 5c). The window reads only the flag "a token is saved", and only while that page is open; it
 * never reads the token (nothing can). Save sends what was typed to Rust, which checks its shape and puts it in the system keychain; Remove asks Rust
 * to delete the item. Errors (`telegram_token_invalid`, `secret_store_unavailable`, `secret_store_denied`) stay on the page as one line.
 */
export function useTelegramToken(api: KinigeApi, active: boolean): TokenControls {
  const [saved, setSaved] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorCode | null>(null);
  // Only the answer of "Remove" says it: the system refused to delete the old item, so it is still in the keychain.
  const [itemLeft, setItemLeft] = useState(false);

  useEffect(() => {
    if (!active) return;
    let alive = true;
    api.getTelegramTokenState().then(
      (state) => {
        if (alive) setSaved(state.saved);
      },
      () => {
        if (alive) setSaved(false);
      },
    );
    return () => {
      alive = false;
    };
  }, [api, active]);

  const run = (action: Promise<TelegramTokenState>) => {
    setBusy(true);
    setError(null);
    setItemLeft(false);
    action
      .then((state) => {
        setSaved(state.saved);
        setItemLeft(state.itemLeft);
      })
      .catch((reason: unknown) => setError(errorCode(reason)))
      .finally(() => setBusy(false));
  };

  return {
    saved,
    busy,
    error,
    itemLeft,
    onSave: (token) => run(api.setTelegramBotToken(token)),
    onRemove: () => run(api.clearTelegramBotToken()),
  };
}
