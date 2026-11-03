import { describe, expect, it } from "vitest";
import { render } from "../../test/render";
import { DropOverlay } from "./DropOverlay";

describe("DropOverlay", () => {
  it("виден: текст и без aria-hidden", () => {
    const { container, getByText } = render(<DropOverlay visible />, { language: "en" });
    expect(getByText("Drop to attach")).toBeInTheDocument();
    expect(container.firstElementChild).toHaveClass("drop-overlay--visible");
    expect(container.firstElementChild).not.toHaveAttribute("aria-hidden");
  });

  it("скрыт: aria-hidden и без класса видимости", () => {
    const { container } = render(<DropOverlay visible={false} />);
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
    expect(container.firstElementChild).not.toHaveClass("drop-overlay--visible");
  });
});
