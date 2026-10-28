import { act, waitFor } from "@testing-library/react";

/** Подмена IntersectionObserver для тестов ленты: тест сам «показывает» край списка. */
export class FakeObserver {
  static all: FakeObserver[] = [];
  private readonly targets = new Set<Element>();
  constructor(private readonly callback: IntersectionObserverCallback) {
    FakeObserver.all.push(this);
  }
  observe(target: Element) {
    this.targets.add(target);
  }
  unobserve(target: Element) {
    this.targets.delete(target);
  }
  disconnect() {
    this.targets.clear();
    FakeObserver.all = FakeObserver.all.filter((o) => o !== this);
  }
  takeRecords() {
    return [];
  }
  fire(target: Element) {
    this.callback([{ isIntersecting: true, target } as IntersectionObserverEntry], this as unknown as IntersectionObserver);
  }
  has(target: Element) {
    return this.targets.has(target);
  }
}

/** В `column-reverse` первый в разметке край стоит внизу (новое), последний — вверху (старое). */
export const edges = (container: HTMLElement) => [...container.querySelectorAll<HTMLElement>(".feed__edge")];

/**
 * Край списка показался в области прокрутки. Ленту наблюдатель ставит эффектом после отрисовки страницы (при смене числа
 * записей прежний наблюдатель снимается): ждём, пока он увидит край, как настоящий браузер сообщил бы о нём не раньше.
 */
export async function showEdge(container: HTMLElement, which: "older" | "newer") {
  const watched = () => {
    const list = edges(container);
    const edge = which === "older" ? list.at(-1) : list[0];
    const observer = FakeObserver.all.find((o) => edge && o.has(edge));
    if (!edge || !observer) throw new Error(`нет края «${which}»`);
    return { edge, observer };
  };
  await waitFor(watched);
  const { edge, observer } = watched();
  act(() => observer.fire(edge));
}
