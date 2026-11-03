import { describe, expect, it, vi } from "vitest";
import { render, screen } from "../../test/render";
import { FOREST_2026 } from "./fixtures";
import { ForestPane, type ForestPaneProps } from "./ForestPane";

const props: ForestPaneProps = {
  view: "strip",
  mode: "forest",
  table: null,
  year: 2026,
  minYear: 2025,
  maxYear: 2026,
  selected: null,
  onOpenDay: vi.fn(),
  onYearChange: vi.fn(),
  onView: vi.fn(),
  onMode: vi.fn(),
  onHide: vi.fn(),
};

describe("the pane before its year has arrived", () => {
  it("holds the strip's room from the first frame: the same grid of invisible cells, hidden from assistive technology", () => {
    const { container } = render(<ForestPane {...props} />, { language: "en" });
    const placeholder = container.querySelector(".forest-strip--placeholder");
    expect(placeholder).toHaveAttribute("aria-hidden", "true");
    // The month row and the seven weekday rows, 53 cells in each of the seven.
    expect(placeholder!.querySelectorAll(".forest-strip__row")).toHaveLength(8);
    expect(placeholder!.querySelectorAll(".forest-strip__day")).toHaveLength(7 * 53);
    expect(screen.queryByRole("grid")).toBeNull();
  });

  it("is replaced by the strip when the table arrives, and not shown for a failed load or in the expanded view", () => {
    const view = render(<ForestPane {...props} />, { language: "en" });
    view.rerender(<ForestPane {...props} table={FOREST_2026} />);
    expect(document.querySelector(".forest-strip--placeholder")).toBeNull();
    expect(screen.getByRole("grid", { name: "Days of 2026" })).toBeInTheDocument();
    view.rerender(<ForestPane {...props} failed />);
    expect(document.querySelector(".forest-strip--placeholder")).toBeNull();
    view.rerender(<ForestPane {...props} view="expanded" />);
    expect(document.querySelector(".forest-strip--placeholder")).toBeNull();
  });
});
