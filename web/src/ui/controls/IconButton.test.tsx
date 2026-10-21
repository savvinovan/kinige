import { fireEvent, render, screen } from "../../test/render";
import { describe, expect, it, vi } from "vitest";
import { IconButton } from "./IconButton";

describe("IconButton", () => {
  it("имя кнопки — label, значок скрыт от диктора", () => {
    const { container } = render(<IconButton icon="attach" label="Прикрепить файл" />);
    expect(screen.getByRole("button", { name: "Прикрепить файл" })).toBeInTheDocument();
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("обычная кнопка вызывает onClick", () => {
    const onClick = vi.fn();
    render(<IconButton icon="emoji" label="Эмодзи" onClick={onClick} />);
    fireEvent.click(screen.getByRole("button", { name: "Эмодзи" }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("недоступная («Появится позже»): объявлена aria-disabled, остаётся в порядке обхода, onClick не вызывается", () => {
    const onClick = vi.fn();
    render(<IconButton icon="voice" label="Записать голосовое" unavailable onClick={onClick} />);
    const button = screen.getByRole("button", { name: "Записать голосовое" });
    expect(button).toHaveAttribute("aria-disabled", "true");
    expect(button).not.toBeDisabled();
    expect(button).toHaveAccessibleDescription("Появится позже");
    button.focus();
    expect(button).toHaveFocus();
    fireEvent.click(button);
    expect(onClick).not.toHaveBeenCalled();
  });
});
