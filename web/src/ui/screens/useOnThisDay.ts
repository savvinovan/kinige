import { useCallback, useEffect, useRef, useState } from "react";
import type { KinigeApi } from "../../api/KinigeApi";
import type { OnThisDay } from "../../api/generated/OnThisDay";

interface UseOnThisDayOptions {
  api: KinigeApi;
  /** Карточка нужна: открыт Inbox, без фильтра по тегу. */
  enabled: boolean;
  /** День журнала изменился, пока окно было открыто: ленту нужно перечитать (метки «Сегодня» и «Вчера»). */
  onNewDay: () => void;
}

/**
 * Карточка «В этот день» Inbox (слайс 5). Читается при открытии Inbox и при фокусе окна: так замечается и смена дня журнала
 * (после 04:00 появляется карточка нового дня), и скрытие с другого места. `ready` — первое чтение закончилось (успехом или
 * ошибкой): лента показывается, только когда карточка уже известна, и в первом кадре ничего не прыгает. Ошибка чтения —
 * карточки просто нет: это украшение, а не данные.
 */
export function useOnThisDay({ api, enabled, onNewDay }: UseOnThisDayOptions) {
  const [card, setCard] = useState<OnThisDay | null>(null);
  const [ready, setReady] = useState(false);
  const shownDay = useRef<string | null>(null);
  const newDay = useRef(onNewDay);
  useEffect(() => {
    newDay.current = onNewDay;
  });

  const refresh = useCallback(() => {
    return api
      .getOnThisDay()
      .then((next) => {
        setCard(next);
        const day = next?.today ?? null;
        if (day !== null && shownDay.current !== null && shownDay.current !== day) newDay.current();
        if (day !== null) shownDay.current = day;
      })
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
