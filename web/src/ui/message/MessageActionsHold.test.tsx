import { act, fireEvent, render } from "../../test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactionView } from "../../api/generated/ReactionView";
import { Message } from "./Message";

// Slice 5a, KINIG-138: the row of an entry that is held (pointer or focus) keeps its place on screen when a put or a removal changes the bubble's height,
// and its `flip` is measured again when focus comes in. jsdom has no layout, so the sizes and the wrapper's top are stubbed.

const sentAt = new Date(2026, 10, 18, 9, 5);
const chip: ReactionView = { emoji: "🔥", putAt: 0, putTz: "UTC", yearsLater: 0 };
const SHIFT = "--actions-shift";
/** How far the row has been moved (the number of pixels in `--actions-shift`), or `null` when the property is not set. */
const shiftOf = (actions: HTMLElement) => {
  const value = actions.style.getPropertyValue(SHIFT);
  return value === "" ? null : Number.parseFloat(value);
};

/** The layout the stubs describe: a wide bubble (the row stands over its top edge) and the wrapper's top relative to the feed. */
const layout = { wrapWidth: 480, wrapTop: 100 };

beforeEach(() => {
  vi.useFakeTimers();
  layout.wrapWidth = 480;
  layout.wrapTop = 100;
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
    if (this.classList.contains("message-wrap")) return layout.wrapWidth;
    return this.classList.contains("message-actions") ? 223 : 0;
  });
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(668);
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (this: HTMLElement) {
    return this.classList.contains("message-actions") ? 26 : 0;
  });
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const y = this.classList.contains("message-wrap") ? layout.wrapTop : 0;
    return { top: y, bottom: y, left: 0, right: 0, width: 0, height: 0, x: 0, y, toJSON: () => ({}) };
  });
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function feed(reactions: ReactionView[]) {
  return (
    <div className="feed-scroll">
      <ul>
        <li className="feed__item">
          <Message markdown="Купить хлеб" sentAt={sentAt} reactions={reactions} reactionKeysHad={[]} onReact={() => undefined} />
        </li>
      </ul>
    </div>
  );
}

const enter = (element: Element) => act(() => void element.dispatchEvent(new Event("pointerenter")));
const leave = (element: Element) => act(() => void element.dispatchEvent(new Event("pointerleave")));
const advance = (ms: number) => act(() => void vi.advanceTimersByTime(ms));

describe("ряд действий удерживает место, когда плашка меняет высоту", () => {
  const setup = (reactions: ReactionView[] = []) => {
    const view = render(feed(reactions));
    const row = view.container.querySelector(".feed__item") as HTMLElement;
    const wrap = view.container.querySelector(".message-wrap") as HTMLElement;
    const actions = () => wrap.querySelector(".message-actions") as HTMLElement;
    return { ...view, row, wrap, actions };
  };

  it("реакция добавляет ряд чипов, плашка растёт вверх на 25 px: ряд сдвигается на 25 px вниз, под указателем остаётся та же кнопка; снятие возвращает", () => {
    const { rerender, row, actions } = setup();
    enter(row);
    expect(shiftOf(actions())).toBeNull();
    layout.wrapTop = 75;
    rerender(feed([chip]));
    expect(shiftOf(actions())).toBe(25);
    layout.wrapTop = 100;
    rerender(feed([]));
    expect(shiftOf(actions())).toBe(0);
  });

  it("прокрутка лишь переносит точку отсчёта: сдвига от неё нет, а рост после неё считается от нового места", () => {
    const { container, rerender, row, actions } = setup();
    enter(row);
    layout.wrapTop = 60;
    act(() => void container.querySelector(".feed-scroll")?.dispatchEvent(new Event("scroll")));
    expect(shiftOf(actions())).toBeNull();
    layout.wrapTop = 35;
    rerender(feed([chip]));
    expect(shiftOf(actions())).toBe(25);
  });

  it("запись у верхнего края ленты: рост плашки не переворачивает ряд внутрь (ряд остаётся там, где его нашёл указатель), а сдвиг считается по-прежнему", () => {
    const { container, rerender, row, actions } = setup();
    const wrap = container.querySelector(".message-wrap") as HTMLElement;
    enter(row);
    expect(wrap).not.toHaveClass("message-wrap--actions-flip"); // 100 px of room above the bubble
    layout.wrapTop = 5; // the bubble grew upwards and now stands close to the feed's top edge
    rerender(feed([chip]));
    expect(wrap).not.toHaveClass("message-wrap--actions-flip");
    expect(shiftOf(actions())).toBe(95);
  });

  it("прокрутка после сдвига начинает размещение заново: сдвиг снят, следующий рост считается от нового места", () => {
    const { container, rerender, row, actions } = setup();
    enter(row);
    layout.wrapTop = 75;
    rerender(feed([chip]));
    expect(shiftOf(actions())).toBe(25);
    act(() => void container.querySelector(".feed-scroll")?.dispatchEvent(new Event("scroll")));
    expect(shiftOf(actions())).toBeNull();
    layout.wrapTop = 50;
    rerender(feed([chip, { ...chip, emoji: "😂" }]));
    expect(shiftOf(actions())).toBe(25);
  });

  it("узкая плашка: ряд справа от неё стоит на нижней линии и не сдвигается", () => {
    layout.wrapWidth = 200;
    const { rerender, row, actions } = setup();
    enter(row);
    layout.wrapTop = 75;
    rerender(feed([chip]));
    expect(shiftOf(actions())).toBe(0);
  });

  it("когда указатель ушёл и задержка прошла, сдвиг снимается; следующее наведение начинает с места по умолчанию", () => {
    const { rerender, row, actions } = setup();
    enter(row);
    layout.wrapTop = 75;
    rerender(feed([chip]));
    expect(shiftOf(actions())).toBe(25);
    leave(row);
    advance(5000);
    expect(shiftOf(actions())).toBeNull();
    enter(row);
    expect(shiftOf(actions())).toBeNull();
  });

  it("фокус внутри держит ряд так же, а потеря фокуса его отпускает", () => {
    const { rerender, wrap, actions } = setup();
    const button = wrap.querySelector("button") as HTMLButtonElement;
    act(() => button.focus());
    layout.wrapTop = 75;
    rerender(feed([chip]));
    expect(shiftOf(actions())).toBe(25);
    act(() => button.blur());
    expect(shiftOf(actions())).toBeNull();
  });
});

describe("ряд, показанный с клавиатуры, не ложится на текст", () => {
  it("запись попала в ленту выше видимой части: при фокусе внутри места над плашкой нет, ряд переворачивается внутрь (раньше это знали только при наведении)", () => {
    layout.wrapTop = 100;
    const { container } = render(feed([]));
    const wrap = container.querySelector(".message-wrap") as HTMLElement;
    expect(wrap).not.toHaveClass("message-wrap--actions-flip");
    layout.wrapTop = 5; // the entry has scrolled to the feed's top edge since it was mounted
    fireEvent.focusIn(wrap.querySelector("button") as HTMLElement);
    expect(wrap).toHaveClass("message-wrap--actions-flip");
  });

  it("прокрутка при удерживаемой записи тоже меряет заново", () => {
    layout.wrapTop = 100;
    const { container } = render(feed([]));
    const wrap = container.querySelector(".message-wrap") as HTMLElement;
    enter(container.querySelector(".feed__item") as HTMLElement);
    expect(wrap).not.toHaveClass("message-wrap--actions-flip");
    layout.wrapTop = 5;
    act(() => void container.querySelector(".feed-scroll")?.dispatchEvent(new Event("scroll")));
    expect(wrap).toHaveClass("message-wrap--actions-flip");
  });
});
