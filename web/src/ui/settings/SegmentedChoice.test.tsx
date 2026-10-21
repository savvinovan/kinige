import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { SegmentedChoice } from "./SegmentedChoice";

const OPTIONS = [
  { value: "normal", label: "Обычный" },
  { value: "large", label: "Крупный" },
] as const;

function Demo({ initial = "normal", onChange = () => {} }: { initial?: "normal" | "large"; onChange?: (value: string) => void }) {
  const [value, setValue] = useState<"normal" | "large">(initial);
  return (
    <>
      <span id="caption">Размер текста</span>
      <SegmentedChoice
        labelledBy="caption"
        options={OPTIONS}
        value={value}
        onChange={(next) => {
          setValue(next);
          onChange(next);
        }}
      />
    </>
  );
}

describe("SegmentedChoice", () => {
  it("radiogroup с именем из заголовка; варианты — radio с aria-checked", () => {
    render(<Demo />);
    expect(screen.getByRole("radiogroup", { name: "Размер текста" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Обычный" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("radio", { name: "Крупный" })).toHaveAttribute("aria-checked", "false");
  });

  it("имя группы можно задать текстом", () => {
    render(<SegmentedChoice label="Размер" options={OPTIONS} value="normal" onChange={() => {}} />);
    expect(screen.getByRole("radiogroup", { name: "Размер" })).toBeInTheDocument();
  });

  it("roving tabindex: в обходе только выбранный вариант", () => {
    render(<Demo initial="large" />);
    expect(screen.getByRole("radio", { name: "Крупный" })).toHaveAttribute("tabindex", "0");
    expect(screen.getByRole("radio", { name: "Обычный" })).toHaveAttribute("tabindex", "-1");
  });

  it("щелчок выбирает вариант", () => {
    const onChange = vi.fn();
    render(<Demo onChange={onChange} />);
    fireEvent.click(screen.getByRole("radio", { name: "Крупный" }));
    expect(onChange).toHaveBeenCalledWith("large");
    expect(screen.getByRole("radio", { name: "Крупный" })).toHaveAttribute("aria-checked", "true");
  });

  it("стрелки двигают фокус и сразу выбирают; по кругу", () => {
    const onChange = vi.fn();
    render(<Demo onChange={onChange} />);
    const normal = screen.getByRole("radio", { name: "Обычный" });
    const large = screen.getByRole("radio", { name: "Крупный" });
    normal.focus();
    fireEvent.keyDown(normal, { key: "ArrowRight" });
    expect(large).toHaveFocus();
    expect(large).toHaveAttribute("aria-checked", "true");
    expect(large).toHaveAttribute("tabindex", "0");
    expect(normal).toHaveAttribute("tabindex", "-1");
    fireEvent.keyDown(large, { key: "ArrowRight" });
    expect(normal).toHaveFocus();
    expect(normal).toHaveAttribute("aria-checked", "true");
    fireEvent.keyDown(normal, { key: "ArrowLeft" });
    expect(large).toHaveFocus();
    fireEvent.keyDown(large, { key: "ArrowUp" });
    expect(normal).toHaveFocus();
    fireEvent.keyDown(normal, { key: "ArrowDown" });
    expect(large).toHaveFocus();
    expect(onChange).toHaveBeenCalledTimes(5);
  });

  it("Home и End выбирают первый и последний вариант", () => {
    render(<Demo />);
    const normal = screen.getByRole("radio", { name: "Обычный" });
    const large = screen.getByRole("radio", { name: "Крупный" });
    normal.focus();
    fireEvent.keyDown(normal, { key: "End" });
    expect(large).toHaveFocus();
    expect(large).toHaveAttribute("aria-checked", "true");
    fireEvent.keyDown(large, { key: "Home" });
    expect(normal).toHaveFocus();
    expect(normal).toHaveAttribute("aria-checked", "true");
  });
});
