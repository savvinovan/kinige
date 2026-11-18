import { describe, expect, it, vi } from "vitest";
import { demoEntry } from "../../api/demoData";
import { fireEvent, render, screen, waitFor } from "../../test/render";
import { Feed, type FeedPaging } from "./Feed";

// "Jump to latest" (KINIG-145): the show and hide rule, the click on a loaded feed and on a window around an old entry, the keyboard path.
const MESSAGES = [
  demoEntry("m1", "2026-10-10", "09:00", "first"),
  demoEntry("m2", "2026-10-11", "09:00", "second"),
  demoEntry("m3", "2026-10-12", "09:00", "third"),
];
const SCREEN = 600;
const NAME = "К последним записям";

const paging = (over: Partial<FeedPaging> = {}): FeedPaging => ({
  hasOlder: false,
  hasNewer: false,
  onLoadOlder: () => undefined,
  onLoadNewer: () => undefined,
  onJumpToLatest: () => Promise.resolve(),
  ...over,
});

/** jsdom has no layout: the scroller is `SCREEN` px tall, and the person scrolls by setting `scrollTop` (negative, as in `column-reverse`). */
function scrollerAt(distance: number) {
  const scroller = screen.getByRole("region", { name: "Записи" });
  Object.defineProperty(scroller, "clientHeight", { configurable: true, value: SCREEN });
  scroller.scrollTop = -distance;
  fireEvent.scroll(scroller);
  return scroller;
}

const button = () => screen.queryByRole("button", { name: NAME });

describe("Feed: the «К последним записям» button shows and hides", () => {
  it("at the bottom edge there is no button", () => {
    render(<Feed messages={MESSAGES} today="2026-10-12" paging={paging()} />);
    expect(button()).toBeNull();
    scrollerAt(0);
    expect(button()).toBeNull();
  });

  it("more than one screen above the bottom edge it appears, one screen or less it goes", () => {
    render(<Feed messages={MESSAGES} today="2026-10-12" paging={paging()} />);
    scrollerAt(SCREEN);
    expect(button()).toBeNull();
    scrollerAt(SCREEN + 1);
    expect(button()).not.toBeNull();
    scrollerAt(SCREEN * 5);
    expect(button()).not.toBeNull();
    scrollerAt(SCREEN - 1);
    expect(button()).toBeNull();
  });

  it("the sign of scrollTop does not matter (engines differ on `column-reverse`)", () => {
    render(<Feed messages={MESSAGES} today="2026-10-12" paging={paging()} />);
    const scroller = screen.getByRole("region", { name: "Записи" });
    Object.defineProperty(scroller, "clientHeight", { configurable: true, value: SCREEN });
    scroller.scrollTop = SCREEN * 2;
    fireEvent.scroll(scroller);
    expect(button()).not.toBeNull();
  });

  it("a feed that holds only a window around an old entry shows it even at the bottom of the window", () => {
    render(<Feed messages={MESSAGES} today="2026-10-12" paging={paging({ hasNewer: true })} />);
    expect(button()).not.toBeNull();
  });

  it("an empty feed and a feed that is still loading have none", () => {
    const { rerender } = render(<Feed messages={[]} today="2026-10-12" paging={paging({ hasNewer: true })} />);
    expect(button()).toBeNull();
    rerender(<Feed messages={MESSAGES} today="2026-10-12" paging={paging({ hasNewer: true })} loading />);
    expect(button()).toBeNull();
  });

  it("the name is in both languages", () => {
    render(<Feed messages={MESSAGES} today="2026-10-12" paging={paging({ hasNewer: true })} />, { language: "en" });
    expect(screen.getByRole("button", { name: "Jump to latest" })).toHaveAttribute("title", "Jump to latest");
  });

  it("it overlays the feed: it is not inside the scrolling area, so the list does not move when it comes", () => {
    render(<Feed messages={MESSAGES} today="2026-10-12" paging={paging({ hasNewer: true })} />);
    const scroller = screen.getByRole("region", { name: "Записи" });
    expect(scroller.contains(button())).toBe(false);
    expect(scroller.parentElement).toHaveClass("feed-pane");
    expect(scroller.parentElement?.contains(button())).toBe(true);
  });
});

describe("Feed: the click", () => {
  it("on a loaded feed: the bottom edge at once, the button goes, no page is requested twice", async () => {
    const onJumpToLatest = vi.fn(() => Promise.resolve());
    render(<Feed messages={MESSAGES} today="2026-10-12" paging={paging({ onJumpToLatest })} />);
    const scroller = scrollerAt(SCREEN * 3);
    fireEvent.click(button() as HTMLElement);
    await waitFor(() => expect(scroller.scrollTop).toBe(0));
    expect(onJumpToLatest).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(button()).toBeNull());
  });

  it("on a window around an old entry: the latest page is awaited first, then the feed goes to its bottom edge", async () => {
    let finish: () => void = () => undefined;
    const onJumpToLatest = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    const { rerender } = render(<Feed messages={MESSAGES} today="2026-10-12" paging={paging({ hasNewer: true, onJumpToLatest })} />);
    const scroller = scrollerAt(SCREEN * 3);
    fireEvent.click(button() as HTMLElement);
    expect(onJumpToLatest).toHaveBeenCalledTimes(1);
    // The latest page has not come yet: the feed stays where it is.
    expect(scroller.scrollTop).toBe(-SCREEN * 3);
    // It comes: the list is the last page and has no newer edge any more.
    rerender(<Feed messages={[...MESSAGES, demoEntry("m4", "2026-10-13", "09:00", "latest")]} today="2026-10-13" paging={paging({ onJumpToLatest })} />);
    finish();
    await waitFor(() => expect(scroller.scrollTop).toBe(0));
    expect(button()).toBeNull();
  });

  it("the button is a real button after the feed in the tab order, so Tab and Enter or Space reach it", () => {
    render(<Feed messages={MESSAGES} today="2026-10-12" paging={paging({ hasNewer: true })} />);
    const scroller = screen.getByRole("region", { name: "Записи" });
    const jump = button() as HTMLButtonElement;
    expect(jump.tagName).toBe("BUTTON");
    expect(jump.type).toBe("button");
    expect(jump.tabIndex).toBe(0);
    expect(jump.hasAttribute("aria-disabled")).toBe(false);
    expect(scroller.compareDocumentPosition(jump) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    jump.focus();
    expect(jump).toHaveFocus();
  });
});
