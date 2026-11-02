import { describe, expect, it, vi } from "vitest";
import { demoEntry } from "../../api/demoData";
import { rect } from "../../test/rect";
import { fireEvent, render, screen, waitFor } from "../../test/render";
import { Feed } from "./Feed";

// The day at the top of the feed (slice 6): reported on a list change and while the person scrolls, and not for the feed's own jumps.
const MESSAGES = [
  demoEntry("m1", "2026-10-10", "09:00", "first day"),
  demoEntry("m2", "2026-10-11", "09:00", "second day"),
  demoEntry("m3", "2026-10-12", "09:00", "third day"),
];

/** jsdom has no layout: the elements of `topDate` start at the top edge of the scroller, earlier ones above, later below. */
function place(topDate: string) {
  const scroller = screen.getByRole("region", { name: "Записи" });
  scroller.getBoundingClientRect = () => rect(0, 100, 600, 400);
  const nodes = [...scroller.querySelectorAll<HTMLElement>("[data-journal-date]")];
  const first = nodes.findIndex((node) => node.dataset.journalDate === topDate);
  nodes.forEach((node, i) => {
    const top = 100 + (i - first) * 60;
    node.getBoundingClientRect = () => rect(0, top, 600, 50);
  });
  return scroller;
}

const nextFrame = () => new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)));

describe("Feed reports the day at its top", () => {
  it("carries the journal date on day labels and entries", () => {
    const { container } = render(<Feed messages={MESSAGES} today="2026-10-12" />);
    const dates = [...container.querySelectorAll("[data-journal-date]")].map((n) => n.getAttribute("data-journal-date"));
    expect(dates).toEqual(["2026-10-10", "2026-10-10", "2026-10-11", "2026-10-11", "2026-10-12", "2026-10-12"]);
  });

  it("reports on scroll once the person has used the wheel, a key, a touch or a press", async () => {
    for (const start of [
      (el: HTMLElement) => fireEvent.wheel(el),
      (el: HTMLElement) => fireEvent.keyDown(el, { key: "PageUp" }),
      (el: HTMLElement) => fireEvent.touchStart(el),
      (el: HTMLElement) => fireEvent.pointerDown(el),
    ]) {
      const onTopDay = vi.fn();
      const { unmount } = render(<Feed messages={MESSAGES} today="2026-10-12" onTopDay={onTopDay} />);
      const scroller = place("2026-10-11");
      start(scroller);
      fireEvent.scroll(scroller);
      await waitFor(() => expect(onTopDay).toHaveBeenLastCalledWith("2026-10-11"));
      unmount();
    }
  });

  it("scroll events come in a burst, one report per frame", async () => {
    const onTopDay = vi.fn();
    render(<Feed messages={MESSAGES} today="2026-10-12" onTopDay={onTopDay} />);
    const scroller = place("2026-10-12");
    onTopDay.mockClear();
    for (let i = 0; i < 20; i++) fireEvent.scroll(scroller);
    await nextFrame();
    await nextFrame();
    expect(onTopDay).toHaveBeenCalledTimes(1);
  });

  it("a jump to an entry is not reported until the person scrolls again", async () => {
    const onTopDay = vi.fn();
    const view = render(<Feed messages={MESSAGES} today="2026-10-12" onTopDay={onTopDay} scrollTo={null} />);
    const scroller = place("2026-10-10");
    view.rerender(<Feed messages={MESSAGES} today="2026-10-12" onTopDay={onTopDay} scrollTo={{ id: "m2", nonce: 1 }} />);
    onTopDay.mockClear();
    fireEvent.scroll(scroller);
    await nextFrame();
    await nextFrame();
    expect(onTopDay).not.toHaveBeenCalled();
    fireEvent.wheel(scroller);
    fireEvent.scroll(scroller);
    await waitFor(() => expect(onTopDay).toHaveBeenCalledWith("2026-10-10"));
  });

  it("the list arriving together with a jump is not reported at its old position", () => {
    const onTopDay = vi.fn();
    const view = render(<Feed messages={MESSAGES.slice(0, 2)} today="2026-10-12" onTopDay={onTopDay} />);
    onTopDay.mockClear();
    view.rerender(<Feed messages={MESSAGES} today="2026-10-12" onTopDay={onTopDay} scrollTo={{ id: "m3", nonce: 1 }} />);
    expect(onTopDay).not.toHaveBeenCalled();
  });

  it("scrolling to the bottom on request is the feed's own move too", async () => {
    const onTopDay = vi.fn();
    const view = render(<Feed messages={MESSAGES} today="2026-10-12" onTopDay={onTopDay} toBottom={0} />);
    const scroller = place("2026-10-10");
    view.rerender(<Feed messages={MESSAGES} today="2026-10-12" onTopDay={onTopDay} toBottom={1} />);
    onTopDay.mockClear();
    fireEvent.scroll(scroller);
    await nextFrame();
    await nextFrame();
    expect(onTopDay).not.toHaveBeenCalled();
  });

  it("works without a handler", async () => {
    render(<Feed messages={MESSAGES} today="2026-10-12" />);
    const scroller = place("2026-10-11");
    fireEvent.wheel(scroller);
    expect(() => fireEvent.scroll(scroller)).not.toThrow();
    await nextFrame();
  });
});
