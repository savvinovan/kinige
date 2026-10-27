import { fireEvent, render, screen } from "../../test/render";
import { describe, expect, it, vi } from "vitest";
import { TagFilterBar } from "./TagFilterBar";

describe("TagFilterBar", () => {
  it("показывает «Показаны записи с #тег» в области с подписью", () => {
    render(<TagFilterBar tag="идея" onClear={vi.fn()} />);
    const region = screen.getByRole("region", { name: "Фильтр по тегу" });
    expect(region).toHaveTextContent("Показаны записи с #идея");
    expect(region.querySelector(".tag-filter__tag")).toHaveTextContent("#идея");
  });

  it("кнопка «Сбросить фильтр» — настоящая кнопка, вызывает onClear мышью и с клавиатуры", () => {
    const onClear = vi.fn();
    render(<TagFilterBar tag="идея" onClear={onClear} />);
    const clear = screen.getByRole("button", { name: "Сбросить фильтр" });
    clear.focus();
    expect(clear).toHaveFocus();
    fireEvent.click(clear);
    expect(onClear).toHaveBeenCalledTimes(1);
  });

  it("Esc не обрабатывается (это дело родителя)", () => {
    const onClear = vi.fn();
    render(<TagFilterBar tag="идея" onClear={onClear} />);
    fireEvent.keyDown(screen.getByRole("button"), { key: "Escape" });
    expect(onClear).not.toHaveBeenCalled();
  });

  it("английский: порядок слов задаёт перевод", () => {
    render(<TagFilterBar tag="ideas" onClear={vi.fn()} />, { language: "en" });
    expect(screen.getByRole("region", { name: "Tag filter" })).toHaveTextContent("Showing entries with #ideas");
    expect(screen.getByRole("button", { name: "Clear filter" })).toBeInTheDocument();
  });

  it("длинный тег остаётся в одной области и переносится", () => {
    render(<TagFilterBar tag={"очень_длинный_тег_".repeat(8)} onClear={vi.fn()} />);
    expect(screen.getByRole("region")).toHaveTextContent("#очень_длинный_тег_");
  });
});
