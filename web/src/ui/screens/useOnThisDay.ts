import { useCallback, useEffect, useState } from "react";
import type { KinigeApi } from "../../api/KinigeApi";
import type { OnThisDay } from "../../api/generated/OnThisDay";

interface UseOnThisDayOptions {
  api: KinigeApi;
  /** Карточка нужна: открыт Inbox, без фильтра по тегу. */
  enabled: boolean;
}

/**
 * Карточка «В этот день» Inbox (слайс 5). Читается при открытии Inbox и при фокусе окна: так замечается и смена дня журнала
 * (после 04:00 появляется карточка нового дня), и скрытие с другого места. `ready` — первое чтение закончилось (успехом или
 * ошибкой): лента показывается, только когда карточка уже известна, и в первом кадре ничего не прыгает. Ошибка чтения —
 * карточки просто нет: это украшение, а не данные.
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

  /** Крестик: скрыть до следующего дня журнала. Карточка уходит после ответа Rust. */
  const dismiss = useCallback(async () => {
    if (card === null) return;
    try {
      await api.dismissOnThisDay(card.today);
      setCard(null);
    } catch {
      // Карточка остаётся на месте: нажать можно ещё раз.
    }
  }, [api, card]);

  return { card: enabled ? card : null, ready: !enabled || ready, dismiss, refresh };
}
