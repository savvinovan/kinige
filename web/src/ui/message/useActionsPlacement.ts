import { useEffect, useLayoutEffect, useRef, useState } from "react";

/** Запасная задержка скрытия (мс), если токен `motion.delay.hide-actions` недоступен (тесты без CSS). */
const FALLBACK_HIDE_DELAY_MS = 180;

/** Задержка скрытия из токена `--motion-delay-hide-actions` («0.18s» после Style Dictionary). Reduce Motion её не меняет. */
export function hideDelayMs(): number {
  const raw = getComputedStyle(document.documentElement).getPropertyValue("--motion-delay-hide-actions").trim();
  const value = Number.parseFloat(raw);
  if (Number.isNaN(value)) return FALLBACK_HIDE_DELAY_MS;
  return raw.endsWith("ms") ? value : value * 1000;
}

/**
 * Над плашкой нет места для ряда (первое сообщение у верхнего края ленты). Ряд над плашкой занимает свою высоту минус то,
 * что заходит внутрь плашки (её верхний отступ). Вне ленты (нет `.feed-scroll`) ограничителя сверху нет.
 */
function noRoomAbove(element: HTMLElement): boolean {
  const scroller = element.closest<HTMLElement>(".feed-scroll");
  const actions = element.querySelector<HTMLElement>(".message-actions");
  const plate = element.querySelector<HTMLElement>(".message");
  if (!scroller || !actions || !plate) return false;
  const above = actions.offsetHeight - (Number.parseFloat(getComputedStyle(plate).paddingTop) || 0);
  return element.getBoundingClientRect().top - scroller.getBoundingClientRect().top < above;
}

/**
 * Где стоит ряд действий и когда он виден.
 *
 * - `inside`: справа от плашки не хватает места (плашка + ряд шире колонки) — ряд переходит на верхнюю границу плашки
 *   у её правого края (чуть выше плашки). Ширина внешнего слоя ряда не зависит от режима, поэтому на границе нет дребезга.
 * - `flip`: над плашкой нет места (первое сообщение у верхнего края ленты) — ряд прижимается внутрь плашки, а не обрезается.
 * - `shown`: зона наведения — вся строка ленты (`.feed__item` во всю колонку), а не только плашка; вне ленты — сама обёртка.
 *   После ухода мыши ряд остаётся ещё `motion.delay.hide-actions` (около 180 мс): можно дойти по диагонали до значка.
 */
export function useActionsPlacement(deps: unknown) {
  const wrap = useRef<HTMLDivElement>(null);
  const [inside, setInside] = useState(false);
  const [flip, setFlip] = useState(false);
  const [shown, setShown] = useState(false);

  useLayoutEffect(() => {
    const element = wrap.current;
    const column = element?.parentElement;
    const actions = element?.querySelector<HTMLElement>(".message-actions");
    if (!element || !column || !actions) return;
    const measure = () => {
      setInside(element.offsetWidth + actions.offsetWidth > column.clientWidth);
      setFlip(noRoomAbove(element));
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(column);
    observer.observe(element);
    return () => observer.disconnect();
  }, [deps]);

  useEffect(() => {
    const element = wrap.current;
    if (!element) return;
    const zone = element.closest<HTMLElement>(".feed__item") ?? element;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const enter = () => {
      clearTimeout(timer);
      setFlip(noRoomAbove(element));
      setShown(true);
    };
    const leave = () => {
      clearTimeout(timer);
      timer = setTimeout(() => setShown(false), hideDelayMs());
    };

    zone.addEventListener("pointerenter", enter);
    zone.addEventListener("pointerleave", leave);
    return () => {
      clearTimeout(timer);
      zone.removeEventListener("pointerenter", enter);
      zone.removeEventListener("pointerleave", leave);
    };
  }, []);

  return { wrap, inside, flip, shown };
}
