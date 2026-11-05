import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { BOX, CENTRE, Harness, drag, image, key, percent, view, wheel, withViewerStage } from "./viewerTestKit";

// KINIG-87: rotation, the reset on another photo, the toolbar names, easing.
withViewerStage();

describe("Lightbox rotation", () => {
  it("R and the button turn the photo a quarter counter-clockwise, cumulatively", () => {
    render(<Harness />, { language: "en" });
    key("r");
    expect(view().turn).toBe(-90);
    fireEvent.click(screen.getByRole("button", { name: "Rotate left" }));
    expect(view().turn).toBe(-180);
    key("R");
    key("к", { code: "KeyR" });
    expect(view().turn).toBe(-360);
    key("r", { metaKey: true });
    expect(view().turn).toBe(-360);
  });

  it("a turned landscape photo fits the stage height, a turned portrait one the width", () => {
    render(<Harness />, { language: "en" });
    // Upright: 1000 x 562.5. Turned: 337.5 x 600, that is the scale 0.375 / 0.625 on the upright box.
    key("r");
    expect(view().scale).toBeCloseTo(0.6, 10);
    expect(image().style.width).toBe(`${BOX[0]}px`);
    key("r");
    expect(view().scale).toBe(1);
    key("r");
    expect(view().scale).toBeCloseTo(0.6, 10);
  });

  it("zoom and pan work in the turned frame", () => {
    render(<Harness />, { language: "en" });
    key("r");
    key("+");
    key("+");
    // 200% of the turned fit: 675 x 1200 on a 1000 x 600 stage: narrower than the stage, so only y can move, by 300 at most.
    expect(view().scale).toBeCloseTo(0.6 * 2, 10);
    drag(image(), CENTRE, { clientX: 900, clientY: 100 });
    expect(view().x).toBe(0);
    expect(view().y).toBe(-260);
    drag(image(), CENTRE, { clientX: 600, clientY: 360 - 5000 });
    expect(view().y).toBe(-300);
  });

  it("the pan is clamped again when a turn changes the frame", () => {
    render(<Harness />, { language: "en" });
    for (let i = 0; i < 3; i += 1) key("+");
    drag(image(), CENTRE, { clientX: 600 + 5000, clientY: 360 });
    const before = view().x;
    expect(before).toBeGreaterThan(0);
    key("r");
    // 300% of the turned photo is 1012.5 wide on a 1000 stage: a pan of a few pixels at most.
    expect(Math.abs(view().x)).toBeLessThanOrEqual(7);
  });

  it("is view only: switching photos and reopening start upright", () => {
    const { unmount } = render(<Harness />, { language: "en" });
    key("r");
    key("+");
    key("ArrowRight");
    expect(view()).toEqual({ x: 0, y: 0, scale: 1, turn: 0 });
    expect(percent()).toBe("100%");
    key("r");
    unmount();
    render(<Harness />, { language: "en" });
    expect(view().turn).toBe(0);
  });

  it("the buttons have names and tooltips with their keys, in both languages", () => {
    const { unmount } = render(<Harness />, { language: "en" });
    expect(screen.getByRole("button", { name: "Rotate left" })).toHaveAttribute("title", "Rotate left (R)");
    expect(screen.getByRole("button", { name: "Zoom in" })).toHaveAttribute("title", "Zoom in (+)");
    expect(screen.getByRole("button", { name: "Zoom out" })).toHaveAttribute("title", "Zoom out (-)");
    expect(screen.getByRole("button", { name: "Fit to window, zoom 100%" })).toHaveAttribute("title", "Fit to window (0)");
    expect(screen.getByRole("button", { name: "Close" })).toHaveAttribute("title", "Close (Esc)");
    expect(screen.getByRole("button", { name: "Previous photo" })).toHaveAttribute("title", "Previous photo (←)");
    expect(screen.getByRole("button", { name: "Next photo" })).toHaveAttribute("title", "Next photo (→)");
    unmount();
    render(<Harness />, { language: "ru" });
    expect(screen.getByRole("button", { name: "Повернуть влево" })).toHaveAttribute("title", "Повернуть влево (R)");
    expect(screen.getByRole("button", { name: "Увеличить" })).toHaveAttribute("title", "Увеличить (+)");
    expect(screen.getByRole("button", { name: "Уменьшить" })).toHaveAttribute("title", "Уменьшить (-)");
    expect(screen.getByRole("button", { name: "Вписать в окно, масштаб 100%" })).toHaveAttribute("title", "Вписать в окно (0)");
    expect(screen.getByRole("button", { name: "Закрыть" })).toHaveAttribute("title", "Закрыть (Esc)");
    expect(screen.getByRole("button", { name: "Предыдущее фото" })).toHaveAttribute("title", "Предыдущее фото (←)");
    expect(screen.getByRole("button", { name: "Следующее фото" })).toHaveAttribute("title", "Следующее фото (→)");
  });

  it("only the steps ease; the wheel and a drag follow the input", () => {
    render(<Harness />, { language: "en" });
    expect(image()).not.toHaveClass("lightbox__image--eased");
    key("+");
    expect(image()).toHaveClass("lightbox__image--eased");
    wheel({ deltaY: -10, ctrlKey: true });
    expect(image()).not.toHaveClass("lightbox__image--eased");
    key("r");
    expect(image()).toHaveClass("lightbox__image--eased");
    drag(image(), CENTRE, { clientX: 700, clientY: 360 });
    expect(image()).not.toHaveClass("lightbox__image--eased");
  });
});

describe("Lightbox without a measured stage", () => {
  it("falls back to CSS fit and still turns", () => {
    vi.restoreAllMocks();
    render(<Harness />, { language: "en" });
    expect(image().style.width).toBe("");
    expect(image().style.transform).toBe("");
    key("r");
    expect(image().style.transform).toContain("rotate(-90deg)");
    key("+");
    expect(percent()).toBe("150%");
  });
});
