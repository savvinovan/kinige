import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "../../test/render";
import { DOUBLE_CLICK_MS } from "./useViewerInput";
import { BOX, CENTRE, Harness, click, dialog, drag, image, key, percent, stage, view, wheel, withViewerStage } from "./viewerTestKit";

// KINIG-87: zoom (wheel, pinch, keys, buttons, double click) and pan.
withViewerStage();

describe("Lightbox zoom", () => {
  it("starts at fit, in a box that fits the stage", () => {
    render(<Harness />, { language: "en" });
    expect(percent()).toBe("100%");
    expect(image().style.width).toBe(`${BOX[0]}px`);
    expect(image().style.height).toBe(`${BOX[1]}px`);
    expect(view()).toEqual({ x: 0, y: 0, scale: 1, turn: 0 });
    expect(screen.getByRole("button", { name: "Zoom out" })).toHaveAttribute("aria-disabled", "true");
  });

  it("Cmd and Ctrl with the wheel zoom, a plain wheel does not", () => {
    render(<Harness />, { language: "en" });
    wheel({ deltaY: -100 });
    expect(view().scale).toBe(1);
    wheel({ deltaY: -100, ctrlKey: true });
    const afterCtrl = view().scale;
    expect(afterCtrl).toBeGreaterThan(1.3);
    wheel({ deltaY: -100, metaKey: true });
    expect(view().scale).toBeGreaterThan(afterCtrl);
    wheel({ deltaY: 100, ctrlKey: true });
    expect(view().scale).toBeCloseTo(afterCtrl, 10);
  });

  it("stops the page zoom of a Ctrl + wheel, and leaves a plain wheel alone", () => {
    render(<Harness />, { language: "en" });
    const zoom = new WheelEvent("wheel", { deltaY: -3, ctrlKey: true, cancelable: true, bubbles: true });
    image().dispatchEvent(zoom);
    expect(zoom.defaultPrevented).toBe(true);
    const plain = new WheelEvent("wheel", { deltaY: -3, cancelable: true, bubbles: true });
    image().dispatchEvent(plain);
    expect(plain.defaultPrevented).toBe(false);
  });

  it("zooms around the cursor: a cursor right of the centre moves the photo left", () => {
    render(<Harness />, { language: "en" });
    key("+");
    key("+");
    key("+");
    wheel({ deltaY: -100, ctrlKey: true, clientX: 800, clientY: 360 });
    const { x, y, scale } = view();
    // Fit scale is folded into the transform: 3x then one notch of the wheel.
    expect(scale).toBeGreaterThan(0.625 * 3);
    // The point under the cursor stays: pan = anchor - (anchor - pan) * ratio, with the anchor at 200 px from the centre.
    const ratio = Math.exp(100 * 0.0035);
    expect(x).toBeCloseTo(200 - 200 * ratio, 6);
    expect(y).toBeCloseTo(0, 6);
  });

  it("copes with a huge or zero wheel delta", () => {
    render(<Harness />, { language: "en" });
    wheel({ deltaY: -1e308, ctrlKey: true });
    expect(Number.isFinite(view().scale)).toBe(true);
    expect(percent()).toMatch(/^\d+%$/);
    wheel({ deltaY: 1e308, ctrlKey: true, deltaMode: 2 });
    wheel({ deltaY: 0, ctrlKey: true });
    expect(Number.isFinite(view().scale)).toBe(true);
    for (let i = 0; i < 40; i += 1) wheel({ deltaY: -1e308, ctrlKey: true });
    expect(percent()).toBe("800%");
    for (let i = 0; i < 80; i += 1) wheel({ deltaY: 1e308, ctrlKey: true });
    expect(percent()).toBe("100%");
  });

  it("the keys + = - and 0", () => {
    render(<Harness />, { language: "en" });
    key("+");
    expect(percent()).toBe("150%");
    key("=");
    expect(percent()).toBe("200%");
    key("-");
    expect(percent()).toBe("150%");
    key("+");
    key("+");
    expect(percent()).toBe("300%");
    key("0");
    expect(percent()).toBe("100%");
    // A key with Cmd, Ctrl or Alt belongs to the system.
    key("+", { metaKey: true });
    key("+", { ctrlKey: true });
    expect(percent()).toBe("100%");
  });

  it("stays between fit and 800%, and mutes the button at the end", () => {
    render(<Harness />, { language: "en" });
    for (let i = 0; i < 12; i += 1) key("+");
    expect(percent()).toBe("800%");
    expect(screen.getByRole("button", { name: "Zoom in" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("button", { name: "Zoom out" })).not.toHaveAttribute("aria-disabled");
    for (let i = 0; i < 12; i += 1) key("-");
    expect(percent()).toBe("100%");
  });

  it("the buttons step the zoom and the percentage resets to fit", () => {
    render(<Harness />, { language: "en" });
    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(percent()).toBe("200%");
    fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    expect(percent()).toBe("150%");
    fireEvent.click(screen.getByRole("button", { name: /^Fit to window, zoom 150%/ }));
    expect(percent()).toBe("100%");
    expect(view()).toEqual({ x: 0, y: 0, scale: 1, turn: 0 });
  });

  it("a double click zooms to 2x at the cursor and the next one returns to fit, without closing", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    const at = { clientX: 800, clientY: 360 };
    click(image(), at);
    vi.advanceTimersByTime(DOUBLE_CLICK_MS / 2);
    click(image(), at);
    expect(percent()).toBe("200%");
    // 2x of a 1000 x 562.5 box overflows the stage by 1000 x 525: the pan moved left by the anchor's offset.
    expect(view().x).toBeCloseTo(-200, 6);
    vi.advanceTimersByTime(DOUBLE_CLICK_MS * 4);
    expect(onClose).not.toHaveBeenCalled();
    click(image(), at);
    vi.advanceTimersByTime(DOUBLE_CLICK_MS / 2);
    click(image(), at);
    expect(percent()).toBe("100%");
    expect(view().x).toBe(0);
    vi.advanceTimersByTime(DOUBLE_CLICK_MS * 4);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("two clicks that are too far apart in time or space are not a double click", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    click(image());
    vi.advanceTimersByTime(DOUBLE_CLICK_MS);
    expect(onClose).toHaveBeenCalledTimes(1);
    click(image(), { clientX: 500, clientY: 300 });
    click(image(), { clientX: 700, clientY: 400 });
    expect(percent()).toBe("100%");
  });

  it("a pinch of WebKit zooms from the zoom it started at", () => {
    render(<Harness />, { language: "en" });
    key("+");
    const gesture = (type: string, scale: number) => {
      const event = Object.assign(new Event(type, { cancelable: true, bubbles: true }), { scale, clientX: 600, clientY: 360 });
      act(() => {
        dialog().dispatchEvent(event);
      });
      return event;
    };
    expect(gesture("gesturestart", 1).defaultPrevented).toBe(true);
    gesture("gesturechange", 2);
    expect(percent()).toBe("300%");
    gesture("gesturechange", 1.2);
    expect(percent()).toBe("180%");
    gesture("gesturechange", Number.NaN);
    expect(percent()).toMatch(/^\d+%$/);
    gesture("gesturechange", 1000);
    expect(percent()).toBe("800%");
  });
});

describe("Lightbox pan", () => {
  it("a drag pans a zoomed photo and the clamp keeps it on the stage", () => {
    render(<Harness />, { language: "en" });
    for (let i = 0; i < 4; i += 1) key("+");
    expect(percent()).toBe("400%");
    drag(image(), CENTRE, { clientX: 650, clientY: 330 });
    expect(view()).toMatchObject({ x: 50, y: -30 });
    // 400%: 4000 x 2250 on a 1000 x 600 stage: half the overflow is 1500 and 825.
    drag(image(), CENTRE, { clientX: 600 + 90000, clientY: 360 + 90000 });
    expect(view()).toMatchObject({ x: 1500, y: 825 });
    drag(image(), CENTRE, { clientX: 600 - 90000, clientY: 360 - 90000 });
    expect(view()).toMatchObject({ x: 1500 - 3000, y: 825 - 1650 });
  });

  it("a drag on the backdrop pans too, and the cursor says so while it lasts", () => {
    render(<Harness />, { language: "en" });
    key("+");
    expect(dialog()).toHaveClass("lightbox--zoomed");
    const init = { button: 0, pointerId: 1 };
    fireEvent.pointerDown(stage(), { ...init, clientX: 120, clientY: 100 });
    fireEvent.pointerMove(stage(), { ...init, clientX: 140, clientY: 100 });
    expect(dialog()).toHaveClass("lightbox--dragging");
    expect(view().x).toBe(20);
    fireEvent.pointerUp(stage(), { ...init, clientX: 140, clientY: 100 });
    expect(dialog()).not.toHaveClass("lightbox--dragging");
  });

  it("a drag at fit size moves nothing", () => {
    render(<Harness />, { language: "en" });
    drag(image(), CENTRE, { clientX: 900, clientY: 500 });
    expect(view().x).toBe(0);
    expect(view().y).toBe(0);
    expect(dialog()).not.toHaveClass("lightbox--zoomed");
  });

  it("zooming out to fit brings the pan back to the centre", () => {
    render(<Harness />, { language: "en" });
    for (let i = 0; i < 4; i += 1) key("+");
    drag(image(), CENTRE, { clientX: 900, clientY: 500 });
    expect(view().x).toBe(300);
    key("0");
    expect(view()).toEqual({ x: 0, y: 0, scale: 1, turn: 0 });
  });

  it("a pointer cancel ends the drag without closing", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    key("+");
    fireEvent.pointerDown(image(), { ...CENTRE, button: 0, pointerId: 1 });
    fireEvent.pointerMove(image(), { clientX: 700, clientY: 360, pointerId: 1 });
    fireEvent.pointerCancel(image(), { pointerId: 1 });
    fireEvent.pointerUp(image(), { clientX: 700, clientY: 360, pointerId: 1 });
    expect(onClose).not.toHaveBeenCalled();
    expect(dialog()).not.toHaveClass("lightbox--dragging");
  });
});
