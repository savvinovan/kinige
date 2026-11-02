import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "../../test/render";
import { ForestEngine } from "./engine";
import { FOREST_2026 } from "./fixtures";
import { ForestBoard } from "./ForestBoard";
import { stubCanvas } from "./testCanvas";

const open = vi.fn();
const setup = (selected: string | null = "2026-10-16", language: "en" | "ru" = "en", table = FOREST_2026) => {
  open.mockClear();
  const view = render(<ForestBoard table={table} selected={selected} zoom={null} onOpenDay={open} />, { language });
  return { ...view, canvas: view.container.querySelector("canvas")! };
};
const live = () => screen.getByRole("status");

beforeEach(() => stubCanvas());
afterEach(() => vi.restoreAllMocks());

describe("the board as one focusable element", () => {
  it("is a canvas labelled 'Forest of 2026' with a hint, and the selected day's card waits in a corner", () => {
    const { canvas } = setup();
    expect(canvas).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("application", { name: "Forest of 2026" })).toBe(canvas);
    expect(canvas).toHaveAccessibleDescription(/Arrow keys move between days/);
    const card = document.querySelector(".day-card--pinned")!;
    expect(card).toHaveTextContent("Fri, October 16, 2026");
  });

  it("is labelled in Russian", () => {
    setup("2026-10-16", "ru");
    expect(screen.getByRole("application", { name: "Лес 2026 года" })).toBeInTheDocument();
  });

  it("no card when the feed shows a day outside the year or no day at all", () => {
    setup("2025-10-16");
    expect(document.querySelector(".day-card")).toBeNull();
  });

  it("no card for a day after today", () => {
    setup("2026-12-24");
    expect(document.querySelector(".day-card")).toBeNull();
  });
});

describe("the keyboard", () => {
  it("left and right are the previous and next day, up and down the previous and next week; each is announced", () => {
    const { canvas } = setup("2026-10-16");
    fireEvent.keyDown(canvas, { key: "ArrowLeft" });
    expect(live()).toHaveTextContent(/^October 15, /);
    fireEvent.keyDown(canvas, { key: "ArrowUp" });
    expect(live()).toHaveTextContent(/^October 8, /);
    fireEvent.keyDown(canvas, { key: "ArrowRight" });
    expect(live()).toHaveTextContent(/^October 9, /);
    fireEvent.keyDown(canvas, { key: "ArrowDown" });
    expect(live()).toHaveTextContent(/^October 16, /);
    expect(document.querySelector(".day-card--pinned")).toHaveTextContent("October 16, 2026");
  });

  it("announces 'October 16, N notes, a stage' as a screen reader needs it", () => {
    const { canvas } = setup("2026-10-17");
    const real = FOREST_2026.days.find((d) => d.date === "2026-10-16")!;
    fireEvent.keyDown(canvas, { key: "ArrowLeft" });
    expect(live().textContent).toMatch(new RegExp(`^October 16, ${real.notes} notes, [a-z ]+$`));
  });

  it("Enter and Space open the keyboard day, or the feed's day when nothing was selected", () => {
    const { canvas } = setup("2026-10-16");
    fireEvent.keyDown(canvas, { key: "Enter" });
    expect(open).toHaveBeenLastCalledWith("2026-10-16");
    fireEvent.keyDown(canvas, { key: "ArrowLeft" });
    fireEvent.keyDown(canvas, { key: " " });
    expect(open).toHaveBeenLastCalledWith("2026-10-15");
  });

  it("Escape clears the keyboard selection and the announcement", () => {
    const { canvas } = setup("2026-10-16");
    fireEvent.keyDown(canvas, { key: "ArrowLeft" });
    expect(live()).not.toBeEmptyDOMElement();
    fireEvent.keyDown(canvas, { key: "Escape" });
    expect(live()).toBeEmptyDOMElement();
    expect(document.querySelector(".day-card--pinned")).toHaveTextContent("October 16, 2026");
  });

  it("starts from the last selectable day when the feed shows no day of this year", () => {
    const { canvas } = setup(null);
    fireEvent.keyDown(canvas, { key: "ArrowLeft" });
    expect(live()).toHaveTextContent(/^October 20, /);
  });

  it("does not leave the year and stops at today", () => {
    const { canvas } = setup("2026-10-21");
    fireEvent.keyDown(canvas, { key: "ArrowRight" });
    fireEvent.keyDown(canvas, { key: "ArrowDown" });
    expect(live()).toHaveTextContent(/^October 21, /);
    const first = setup("2026-01-01");
    fireEvent.keyDown(first.canvas, { key: "ArrowLeft" });
    expect(first.container.querySelector('[role="status"]')).toHaveTextContent(/^January 1, /);
  });

  it("leaving the canvas drops the keyboard selection; other keys are not handled", () => {
    const { canvas } = setup("2026-10-16");
    fireEvent.keyDown(canvas, { key: "ArrowLeft" });
    expect(live()).not.toBeEmptyDOMElement();
    fireEvent.blur(canvas);
    expect(live()).toBeEmptyDOMElement();
    const notHandled = fireEvent.keyDown(canvas, { key: "a" });
    expect(notHandled).toBe(true);
  });

  it("when the feed moves to another day, the keyboard selection is dropped", () => {
    const { canvas, rerender } = setup("2026-10-16");
    fireEvent.keyDown(canvas, { key: "ArrowLeft" });
    expect(live()).not.toBeEmptyDOMElement();
    rerender(<ForestBoard table={FOREST_2026} selected="2026-09-01" zoom={null} onOpenDay={open} />);
    expect(live()).toBeEmptyDOMElement();
    expect(document.querySelector(".day-card--pinned")).toHaveTextContent("September 1, 2026");
  });
});

describe("the pointer", () => {
  it("shows the card of the hovered day with its first line, and hides it when the pointer leaves", () => {
    const { canvas } = setup(null);
    // The engine fits the year into the 1280 x 360 canvas: the centre of the canvas is on the board.
    let seen: string | null = null;
    for (let x = 100; x < 1200 && !seen; x += 7) {
      for (let y = 120; y < 340 && !seen; y += 5) {
        act(() => {
          canvas.dispatchEvent(new PointerEvent("pointermove", { clientX: x, clientY: y, pointerId: 1, bubbles: true }));
        });
        const card = document.querySelector(".day-card");
        if (card) seen = card.querySelector(".day-card__date")?.textContent ?? null;
      }
    }
    expect(seen).toMatch(/2026$/);
    act(() => {
      canvas.dispatchEvent(new PointerEvent("pointerleave", { pointerId: 1 }));
    });
    expect(document.querySelector(".day-card")).toBeNull();
  });
});

describe("the header's zoom buttons", () => {
  it("a request fits the whole year or the cells of a season, once per request", () => {
    const year = vi.spyOn(ForestEngine.prototype, "fitYear");
    const cells = vi.spyOn(ForestEngine.prototype, "fitCells");
    const view = render(<ForestBoard table={FOREST_2026} selected={null} zoom={null} onOpenDay={open} />, { language: "en" });
    const baseline = year.mock.calls.length;
    view.rerender(<ForestBoard table={FOREST_2026} selected={null} zoom={{ target: "autumn", nonce: 1 }} onOpenDay={open} />);
    expect(cells).toHaveBeenCalledTimes(1);
    expect(cells.mock.calls[0]?.[0]).toHaveLength(30 + 31 + 30);
    view.rerender(<ForestBoard table={FOREST_2026} selected="2026-10-16" zoom={{ target: "autumn", nonce: 1 }} onOpenDay={open} />);
    expect(cells).toHaveBeenCalledTimes(1);
    view.rerender(<ForestBoard table={FOREST_2026} selected={null} zoom={{ target: "year", nonce: 2 }} onOpenDay={open} />);
    expect(year.mock.calls.length).toBe(baseline + 1);
    view.rerender(<ForestBoard table={FOREST_2026} selected={null} zoom={{ target: "winter", nonce: 3 }} onOpenDay={open} />);
    const winter = cells.mock.calls.at(-1)?.[0] ?? [];
    expect(winter.length).toBe(31 + 28);
  });
});
