import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "../../test/render";
import { DOUBLE_CLICK_MS } from "./useViewerInput";
import { BAD, CENTRE, Harness, click, dialog, drag, image, key, stage, view, wheel, withViewerStage } from "./viewerTestKit";

// KINIG-87: what closes the viewer and what must not.
withViewerStage();

describe("Lightbox closing by click", () => {
  it("closes at once on a click on the backdrop: the scrim, the empty stage, the empty bar", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    click(document.querySelector(".lightbox__scrim") as Element, { clientX: 10, clientY: 10 });
    expect(onClose).toHaveBeenCalledTimes(1);
    click(stage(), { clientX: 120, clientY: 100 });
    click(document.querySelector(".lightbox__bar") as Element, { clientX: 300, clientY: 20 });
    click(dialog(), { clientX: 5, clientY: 400 });
    expect(onClose).toHaveBeenCalledTimes(4);
  });

  it("closes on a click on the photo at fit size, after the double click window", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    click(image());
    expect(onClose).not.toHaveBeenCalled();
    vi.advanceTimersByTime(DOUBLE_CLICK_MS);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("does not close after a drag, and a drag of less than 4 px is still a click", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    drag(stage(), CENTRE, { clientX: 640, clientY: 360 });
    drag(image(), CENTRE, { clientX: 600, clientY: 420 });
    vi.advanceTimersByTime(DOUBLE_CLICK_MS * 2);
    expect(onClose).not.toHaveBeenCalled();
    drag(stage(), { clientX: 120, clientY: 100 }, { clientX: 122, clientY: 102 });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("never closes on the toolbar, the arrows, the counter or Open in default app", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} start={1} />, { language: "en" });
    const controls = [
      screen.getByText("2 / 3"),
      screen.getByRole("button", { name: "Open in default app" }),
      screen.getByRole("button", { name: "Rotate left" }),
      screen.getByRole("button", { name: "Zoom in" }),
      screen.getByRole("button", { name: "Zoom out" }),
      screen.getByRole("button", { name: /^Fit to window, zoom/ }),
      screen.getByRole("button", { name: "Next photo" }),
      screen.getByRole("button", { name: "Previous photo" }),
      document.querySelector(".lightbox__tools") as Element,
    ];
    for (const control of controls) {
      click(control, { clientX: 300, clientY: 30 });
      fireEvent.click(control);
    }
    vi.advanceTimersByTime(DOUBLE_CLICK_MS * 2);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("a press that starts on a button and ends on the backdrop does not close", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    fireEvent.pointerDown(screen.getByRole("button", { name: "Zoom in" }), { ...CENTRE, button: 0, pointerId: 1 });
    fireEvent.pointerUp(stage(), { ...CENTRE, button: 0, pointerId: 1 });
    expect(onClose).not.toHaveBeenCalled();
  });

  it("ignores the right button and a second pointer", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    fireEvent.pointerDown(stage(), { ...CENTRE, button: 2, pointerId: 1 });
    fireEvent.pointerUp(stage(), { ...CENTRE, button: 2, pointerId: 1 });
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.pointerDown(stage(), { ...CENTRE, button: 0, pointerId: 1 });
    fireEvent.pointerDown(stage(), { ...CENTRE, button: 0, pointerId: 2 });
    fireEvent.pointerUp(stage(), { ...CENTRE, button: 0, pointerId: 2 });
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.pointerUp(stage(), { ...CENTRE, button: 0, pointerId: 1 });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("Escape still closes, and a click on the zoomed photo does not, but the backdrop still does", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    key("+");
    click(image());
    vi.advanceTimersByTime(DOUBLE_CLICK_MS * 2);
    expect(onClose).not.toHaveBeenCalled();
    click(stage(), { clientX: 120, clientY: 100 });
    expect(onClose).toHaveBeenCalledTimes(1);
    key("Escape");
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("a pending close dies with the photo: switching photos cancels it", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    click(image());
    key("ArrowRight");
    vi.advanceTimersByTime(DOUBLE_CLICK_MS * 2);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("the 'No preview' tile closes on a click and has no zoom or rotate buttons", () => {
    const onClose = vi.fn();
    render(<Harness images={[BAD]} onClose={onClose} />, { language: "en" });
    fireEvent.error(screen.getByRole("img"));
    fireEvent.error(screen.getByRole("img"));
    expect(screen.getByText("No preview")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Rotate left" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Zoom in" })).toBeNull();
    key("+");
    key("r");
    click(screen.getByText("No preview"));
    vi.advanceTimersByTime(DOUBLE_CLICK_MS);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

// A click on the photo at fit size closes after the double click window. Whoever zooms or turns the photo inside that window did not mean
// to close (the first review of KINIG-87 found the viewer closing under a person who clicked and then pressed +).
describe("Lightbox: a pending close dies when the photo is zoomed or turned", () => {
  const bar = () => document.querySelector(".lightbox__bar") as Element;

  function stayOpenAfter(act: () => void) {
    const onClose = vi.fn();
    const { unmount } = render(<Harness onClose={onClose} />, { language: "en" });
    click(image());
    vi.advanceTimersByTime(DOUBLE_CLICK_MS / 3);
    act();
    vi.advanceTimersByTime(DOUBLE_CLICK_MS * 3);
    expect(onClose).not.toHaveBeenCalled();
    unmount();
  }

  it("a key: + and = zoom, R turns", () => {
    for (const name of ["+", "=", "r"]) stayOpenAfter(() => key(name));
  });

  it("the Zoom in button and the Rotate button", () => {
    // A real press is the pointer events and then the browser's `click`.
    for (const name of ["Zoom in", "Rotate left"]) {
      stayOpenAfter(() => {
        const button = screen.getByRole("button", { name });
        click(button, { clientX: 900, clientY: 30 });
        fireEvent.click(button);
      });
    }
  });

  it("Cmd and Ctrl with the wheel, and a pinch", () => {
    stayOpenAfter(() => wheel({ deltaY: -100, metaKey: true }));
    stayOpenAfter(() => wheel({ deltaY: -100, ctrlKey: true }));
    stayOpenAfter(() => {
      const start = Object.assign(new Event("gesturestart", { cancelable: true, bubbles: true }), { scale: 1 });
      const change = Object.assign(new Event("gesturechange", { cancelable: true, bubbles: true }), { scale: 2 });
      act(() => {
        dialog().dispatchEvent(start);
        dialog().dispatchEvent(change);
      });
    });
  });

  it("a zoom that is undone inside the window still cancels the close; the same click with no change closes", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    click(image());
    key("+");
    key("0");
    vi.advanceTimersByTime(DOUBLE_CLICK_MS * 3);
    expect(onClose).not.toHaveBeenCalled();
    // And without any change the same click still closes.
    click(image());
    vi.advanceTimersByTime(DOUBLE_CLICK_MS);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("the delay is 300 ms: nothing closes at 299, the viewer closes at 300", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    expect(DOUBLE_CLICK_MS).toBe(300);
    click(image());
    vi.advanceTimersByTime(299);
    expect(onClose).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("the empty part of the top bar is the photo while it is zoomed: a click there does not close, at fit it does", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    click(bar(), { clientX: 300, clientY: 20 });
    expect(onClose).toHaveBeenCalledTimes(1);
    key("+");
    click(bar(), { clientX: 300, clientY: 20 });
    vi.advanceTimersByTime(DOUBLE_CLICK_MS * 3);
    expect(onClose).toHaveBeenCalledTimes(1);
    // A drag that starts on the bar pans the zoomed photo like any other drag.
    key("+");
    key("+");
    const before = view().x;
    drag(bar(), { clientX: 300, clientY: 20 }, { clientX: 340, clientY: 20 });
    expect(view().x).toBe(before + 40);
    // The backdrop of a zoomed photo (outside the bar) still closes.
    click(stage(), { clientX: 120, clientY: 100 });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("a button of the bar never closes, zoomed or not", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    key("+");
    click(screen.getByRole("button", { name: "Open in default app" }), { clientX: 300, clientY: 20 });
    vi.advanceTimersByTime(DOUBLE_CLICK_MS * 3);
    expect(onClose).not.toHaveBeenCalled();
  });
});
