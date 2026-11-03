import { useCallback, useEffect, useState } from "react";
import type { KinigeApi } from "../../api/KinigeApi";
import type { OnThisDay } from "../../api/generated/OnThisDay";

interface UseOnThisDayOptions {
  api: KinigeApi;
  /** The card is needed: Inbox is open, with no tag filter. */
  enabled: boolean;
}

/**
 * The "On this day" card of Inbox (slice 5). It is read when Inbox opens and on window focus: this way both a change of
 * the journal day (after 04:00 the new day's card appears) and a dismissal from elsewhere are noticed. `ready` means the
 * first read has finished (with success or an error): the feed is shown only when the card is already known, so nothing
 * jumps in the first frame. A read error just means there is no card: it is a decoration, not data.
 */
export function useOnThisDay({ api, enabled }: UseOnThisDayOptions) {
  const [card, setCard] = useState<OnThisDay | null>(null);
  const [ready, setReady] = useState(false);

  const refresh = useCallback(() => {
    return api
      .getOnThisDay()
      .then(setCard)
      .catch(() => setCard(null))
      .finally(() => setReady(true));
  }, [api]);

  useEffect(() => {
    if (!enabled) return;
    void refresh();
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [enabled, refresh]);

  /**
   * The cross: hide until the next journal day. The card goes away after Rust replies. `true` means hidden; `false` means
   * there is no card or Rust refused: it stays in place and can be pressed again (the caller moves nothing).
   */
  const dismiss = useCallback(async (): Promise<boolean> => {
    if (card === null) return false;
    try {
      await api.dismissOnThisDay(card.today);
      setCard(null);
      return true;
    } catch {
      return false;
    }
  }, [api, card]);

  return { card: enabled ? card : null, ready: !enabled || ready, dismiss, refresh };
}
