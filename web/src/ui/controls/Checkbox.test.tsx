import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { Checkbox } from "./Checkbox";
import { ProgressBar } from "./ProgressBar";

describe("Checkbox", () => {
  it("настоящий флажок: имя — подпись, описание — описание, пробел и щелчок переключают", () => {
    const onChange = vi.fn();
    render(
      <Checkbox description="12 записей" onChange={onChange}>
        Избранное
      </Checkbox>,
    );
    const box = screen.getByRole("checkbox", { name: "Избранное" });
    expect(box).toHaveAccessibleDescription("12 записей");
    expect(box).not.toBeChecked();
    fireEvent.click(box);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(box).toBeChecked();
    fireEvent.click(screen.getByText("Избранное"));
    expect(box).not.toBeChecked();
  });

  it("описание переносится по словам, а не обрезается многоточием (длинное русское пояснение не теряет смысл)", () => {
    const css = readFileSync(resolve(__dirname, "controls.css"), "utf8");
    const rule = /\.checkbox__description \{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(rule).not.toBe("");
    expect(rule).not.toMatch(/white-space:\s*nowrap|text-overflow|overflow:\s*hidden/);
    expect(rule).toMatch(/overflow-wrap:\s*anywhere/);
  });

  it("попадает в порядок Tab и берёт фокус", () => {
    render(<Checkbox>Личный чат</Checkbox>);
    const box = screen.getByRole("checkbox");
    box.focus();
    expect(box).toHaveFocus();
    expect(box).toHaveAttribute("type", "checkbox");
  });

  it("недоступный объявлен недоступным и в Tab не попадает", () => {
    render(<Checkbox disabled>Пустой чат</Checkbox>);
    const box = screen.getByRole("checkbox", { name: "Пустой чат" });
    expect(box).toBeDisabled();
    box.focus();
    expect(box).not.toHaveFocus();
  });

  it("квадрат скрыт от диктора", () => {
    const { container } = render(<Checkbox>Чат</Checkbox>);
    expect(container.querySelector(".checkbox__box")).toHaveAttribute("aria-hidden", "true");
  });
});

describe("ProgressBar", () => {
  it("role=progressbar: имя, границы, значение и текст значения", () => {
    render(<ProgressBar value={3200} max={15000} label="Импорт" valueText="3 200 из 15 000" />);
    const bar = screen.getByRole("progressbar", { name: "Импорт" });
    expect(bar).toHaveAttribute("aria-valuemin", "0");
    expect(bar).toHaveAttribute("aria-valuemax", "15000");
    expect(bar).toHaveAttribute("aria-valuenow", "3200");
    expect(bar).toHaveAttribute("aria-valuetext", "3 200 из 15 000");
  });

  it("заливка — доля от целого; выход за границы и нулевое целое не ломают полосу", () => {
    const { container, rerender } = render(<ProgressBar value={1} max={4} label="x" />);
    const fill = () => container.querySelector<HTMLElement>(".progress__fill")?.style.width;
    expect(fill()).toBe("25%");
    rerender(<ProgressBar value={9} max={4} label="x" />);
    expect(fill()).toBe("100%");
    rerender(<ProgressBar value={-3} max={4} label="x" />);
    expect(fill()).toBe("0%");
    rerender(<ProgressBar value={5} max={0} label="x" />);
    expect(fill()).toBe("0%");
  });
});
