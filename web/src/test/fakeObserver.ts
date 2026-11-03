import { act, waitFor } from "@testing-library/react";

/** An IntersectionObserver replacement for feed tests: the test itself "shows" the list edge. */
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

/** In `column-reverse` the first edge in the markup is at the bottom (new), the last is at the top (old). */
export const edges = (container: HTMLElement) => [...container.querySelectorAll<HTMLElement>(".feed__edge")];

/**
 * The list edge became visible in the scroll area. The observer is attached to the feed by an effect after the page is
 * rendered (when the number of entries changes, the previous observer is removed): we wait until it sees the edge, as a
 * real browser would not report it earlier.
 */
export async function showEdge(container: HTMLElement, which: "older" | "newer") {
  const watched = () => {
    const list = edges(container);
    const edge = which === "older" ? list.at(-1) : list[0];
    const observer = FakeObserver.all.find((o) => edge && o.has(edge));
    if (!edge || !observer) throw new Error(`no "${which}" edge`);
    return { edge, observer };
  };
  await waitFor(watched);
  const { edge, observer } = watched();
  act(() => observer.fire(edge));
}
