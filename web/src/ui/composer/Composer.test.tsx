import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Composer, insideCodeFence } from "./Composer";

const setup = (props: Partial<React.ComponentProps<typeof Composer>> = {}) => {
  const onChange = vi.fn();
  const onSubmit = vi.fn();
  const view = render(<Composer value="" onChange={onChange} onSubmit={onSubmit} {...props} />);
  return { onChange, onSubmit, input: screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement, ...view };
};

describe("Composer: вид и доступность", () => {
  it("поле с подсказкой «Написать заметку…» и три кнопки с подписями", () => {
    const { input } = setup();
    expect(input).toHaveAttribute("placeholder", "Написать заметку…");
    expect(screen.getByRole("button", { name: "Прикрепить файл" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Эмодзи" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Записать голосовое" })).toBeInTheDocument();
  });

  it("кнопки только для вида: aria-disabled, пояснение «Появится позже», нажатие ничего не делает", () => {
    render(<Composer value="" onChange={vi.fn()} viewOnly />);
    const emoji = screen.getByRole("button", { name: "Эмодзи" });
    expect(emoji).toHaveAttribute("aria-disabled", "true");
    expect(emoji).not.toBeDisabled();
    expect(emoji).toHaveAccessibleDescription("Появится позже");
    expect(emoji).toHaveAttribute("title", "Эмодзи. Появится позже");
  });

  it("без viewOnly кнопки обычные: без aria-disabled", () => {
    setup();
    expect(screen.getByRole("button", { name: "Эмодзи" })).not.toHaveAttribute("aria-disabled");
  });

  it("подсказки про Enter и Shift+Enter нет", () => {
    setup();
    expect(screen.queryByText(/Enter/)).toBeNull();
  });

  it("какие кнопки показывать — решает вызывающий (в приложении кнопка есть, когда есть её функция)", () => {
    setup({ buttons: { emoji: true } });
    expect(screen.queryByRole("button", { name: "Прикрепить файл" })).toBeNull();
    expect(screen.getByRole("button", { name: "Эмодзи" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Записать голосовое" })).toBeNull();
  });

  it("все кнопки видны диктору и берут фокус с клавиатуры (обычные <button>)", () => {
    setup();
    for (const button of screen.getAllByRole("button")) {
      expect(button.tagName).toBe("BUTTON");
      button.focus();
      expect(button).toHaveFocus();
    }
  });

  it("ошибка отправки: role=alert, поле помечено недопустимым и связано с текстом ошибки", () => {
    const { input } = setup({ value: "черновик", error: "Не удалось отправить." });
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Не удалось отправить.");
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input.getAttribute("aria-describedby")).toBe(alert.id);
    expect(input).toHaveValue("черновик");
  });

  it("высоту растущего поля задаёт копия текста в атрибуте", () => {
    const { container } = setup({ value: "строка 1\nстрока 2" });
    expect(container.querySelector(".composer__field")).toHaveAttribute("data-replicated-value", "строка 1\nстрока 2");
  });
});

describe("Composer: клавиши", () => {
  it("Enter отправляет непустой черновик", () => {
    const { input, onSubmit } = setup({ value: "купить хлеб" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("Shift+Enter — новая строка, не отправка", () => {
    const { input, onSubmit } = setup({ value: "купить хлеб" });
    expect(fireEvent.keyDown(input, { key: "Enter", shiftKey: true })).toBe(true);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("Enter в пустом или пробельном поле не отправляет, но и не вставляет перенос", () => {
    const { input, onSubmit } = setup({ value: "  \n " });
    expect(fireEvent.keyDown(input, { key: "Enter" })).toBe(false);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("Enter во время ввода через IME подтверждает ввод, а не отправляет", () => {
    const { input, onSubmit } = setup({ value: "приве" });
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("Enter внутри блока кода — новая строка", () => {
    const value = "```\nfn main() {";
    const { input, onSubmit } = setup({ value });
    input.setSelectionRange(value.length, value.length);
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("без onSubmit Enter ничего не делает (D1: поле показывает вид)", () => {
    const { input } = setup({ value: "текст", onSubmit: undefined });
    expect(fireEvent.keyDown(input, { key: "Enter" })).toBe(true);
  });

  it("ввод передаётся наверх", () => {
    const { input, onChange } = setup();
    fireEvent.change(input, { target: { value: "привет" } });
    expect(onChange).toHaveBeenCalledWith("привет");
  });
});

describe("insideCodeFence", () => {
  it("нечётное число ограждений до курсора — внутри блока", () => {
    expect(insideCodeFence("```\nкод", 7)).toBe(true);
    expect(insideCodeFence("```\nкод\n```", 11)).toBe(false);
    expect(insideCodeFence("```\nкод\n```\nтекст ```", 21)).toBe(false);
    expect(insideCodeFence("текст", 5)).toBe(false);
  });

  it("считаются только ограждения в начале строки, курсор перед ограждением — вне блока", () => {
    expect(insideCodeFence("a ``` b", 7)).toBe(false);
    expect(insideCodeFence("текст\n```", 5)).toBe(false);
  });
});

describe("Composer: «Отправить» вместо «микрофона»", () => {
  it("пустое поле и пробелы: «микрофон», «Отправить» нет", () => {
    const { rerender } = render(<Composer value="" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Записать голосовое" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Отправить" })).toBeNull();
    rerender(<Composer value={"  \n "} onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Записать голосовое" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Отправить" })).toBeNull();
  });

  it("есть текст: «микрофон» заменяется кнопкой «Отправить», место одно и то же", () => {
    const { container, rerender } = render(<Composer value="" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(container.querySelectorAll(".composer__action")).toHaveLength(1);
    rerender(<Composer value="купить хлеб" onChange={vi.fn()} onSubmit={vi.fn()} />);
    expect(screen.queryByRole("button", { name: "Записать голосовое" })).toBeNull();
    expect(screen.getByRole("button", { name: "Отправить" })).toBeInTheDocument();
    expect(container.querySelectorAll(".composer__action")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Отправить" })).toHaveClass("icon-button--accent");
  });

  it("нажатие делает то же, что Enter: вызывает onSubmit и возвращает фокус в поле", () => {
    const onSubmit = vi.fn();
    render(<Composer value="купить хлеб" onChange={vi.fn()} onSubmit={onSubmit} />);
    const button = screen.getByRole("button", { name: "Отправить" });
    button.focus();
    fireEvent.click(button);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("textbox", { name: "Новая заметка" })).toHaveFocus();
  });

  it("без onSubmit (D1) кнопка объявлена недоступной и ничего не отправляет", () => {
    render(<Composer value="купить хлеб" onChange={vi.fn()} />);
    const button = screen.getByRole("button", { name: "Отправить" });
    expect(button).toHaveAttribute("aria-disabled", "true");
    expect(button).toHaveAccessibleDescription("Появится позже");
  });

  it("без кнопки «микрофон» при пустом поле правого места нет, с текстом «Отправить» есть", () => {
    const { container, rerender } = render(<Composer value="" onChange={vi.fn()} onSubmit={vi.fn()} buttons={{ emoji: true }} />);
    expect(container.querySelector(".composer__action")).toBeNull();
    rerender(<Composer value="текст" onChange={vi.fn()} onSubmit={vi.fn()} buttons={{ emoji: true }} />);
    expect(screen.getByRole("button", { name: "Отправить" })).toBeInTheDocument();
  });
});
