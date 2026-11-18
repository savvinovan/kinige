import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { ForestToggle } from "./ForestToggle";

describe("the forest switch of the top bar", () => {
  it("is pressed while the forest is shown, with a fixed name and a tooltip that names the action", () => {
    const { rerender } = render(<ForestToggle hidden={false} onToggle={vi.fn()} />, { language: "en" });
    const button = screen.getByRole("button", { name: "Forest" });
    expect(button).toHaveAttribute("aria-pressed", "true");
    expect(button).toHaveAttribute("title", "Hide forest");
    rerender(<ForestToggle hidden onToggle={vi.fn()} />);
    // The same button with the same name: only the state and the tooltip change.
    expect(screen.getByRole("button", { name: "Forest" })).toBe(button);
    expect(button).toHaveAttribute("aria-pressed", "false");
    expect(button).toHaveAttribute("title", "Show forest");
  });

  it("is named in Russian", () => {
    render(<ForestToggle hidden onToggle={vi.fn()} />, { language: "ru" });
    const button = screen.getByRole("button", { name: "Лес" });
    expect(button).toHaveAttribute("title", "Показать лес");
  });

  it("calls the switch once per click and is a real button for the keyboard", () => {
    const onToggle = vi.fn();
    render(<ForestToggle hidden={false} onToggle={onToggle} />, { language: "en" });
    const button = screen.getByRole("button", { name: "Forest" });
    expect(button.tagName).toBe("BUTTON");
    expect(button).not.toHaveAttribute("tabindex", "-1");
    fireEvent.click(button);
    expect(onToggle).toHaveBeenCalledTimes(1);
  });
});
