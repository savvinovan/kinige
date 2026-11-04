import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { DOUBLE_CLICK_MS } from "./useViewerInput";
import { BAD, CENTRE, Harness, click, dialog, drag, image, key, stage, withViewerStage } from "./viewerTestKit";

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
