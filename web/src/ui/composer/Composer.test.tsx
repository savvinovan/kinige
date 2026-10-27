import { fireEvent, render, screen } from "../../test/render";
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

describe("Composer: ⌘Enter / Ctrl+Enter «дописать» (окно захвата, слайс 2)", () => {
  const withAppend = (props: Partial<React.ComponentProps<typeof Composer>> = {}) => {
    const onAppend = vi.fn();
    return { onAppend, ...setup({ value: "и ещё", onAppend, ...props }) };
  };

  it("⌘Enter вызывает onAppend, а не onSubmit, и не вставляет перевод строки", () => {
    const { input, onAppend, onSubmit } = withAppend();
    expect(fireEvent.keyDown(input, { key: "Enter", metaKey: true })).toBe(false);
    expect(onAppend).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("Ctrl+Enter (не macOS) делает то же", () => {
    const { input, onAppend, onSubmit } = withAppend();
    expect(fireEvent.keyDown(input, { key: "Enter", ctrlKey: true })).toBe(false);
    expect(onAppend).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("обычный Enter по-прежнему отправляет, onAppend не вызывается", () => {
    const { input, onAppend, onSubmit } = withAppend();
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onAppend).not.toHaveBeenCalled();
  });

  it("⌘Shift+Enter — не «дописать»: Shift оставляет перевод строки", () => {
    const { input, onAppend, onSubmit } = withAppend();
    expect(fireEvent.keyDown(input, { key: "Enter", metaKey: true, shiftKey: true })).toBe(true);
    expect(onAppend).not.toHaveBeenCalled();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("во время ввода через IME ⌘Enter ничего не делает", () => {
    const { input, onAppend } = withAppend();
    fireEvent.keyDown(input, { key: "Enter", metaKey: true, isComposing: true });
    expect(onAppend).not.toHaveBeenCalled();
  });

  it("внутри блока кода ⌘Enter тоже дописывает и не вставляет перевод строки", () => {
    const value = "```\nfn main() {";
    const { input, onAppend, onSubmit } = withAppend({ value });
    input.setSelectionRange(value.length, value.length);
    expect(fireEvent.keyDown(input, { key: "Enter", metaKey: true })).toBe(false);
    expect(onAppend).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("в пустом или пробельном поле ⌘Enter ничего не дописывает, перевода строки нет", () => {
    const { input, onAppend } = withAppend({ value: " \n " });
    expect(fireEvent.keyDown(input, { key: "Enter", metaKey: true })).toBe(false);
    expect(onAppend).not.toHaveBeenCalled();
  });

  it("без onAppend ⌘Enter и Ctrl+Enter ведут себя как обычный Enter: отправляют", () => {
    const { input, onSubmit } = setup({ value: "текст" });
    fireEvent.keyDown(input, { key: "Enter", metaKey: true });
    fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });
    expect(onSubmit).toHaveBeenCalledTimes(2);
  });

  it("onAppend работает и без onSubmit", () => {
    const onAppend = vi.fn();
    render(<Composer value="текст" onChange={vi.fn()} onAppend={onAppend} />);
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Новая заметка" }), { key: "Enter", metaKey: true });
    expect(onAppend).toHaveBeenCalledTimes(1);
  });
});

describe("Composer: обычный вид — ошибка над полем", () => {
  it("без ошибки области alert нет (как в D1); с ошибкой она над полем, поле описано ею и помечено недопустимым", () => {
    const { rerender, input, container } = setup();
    expect(screen.queryByRole("alert")).toBeNull();
    rerender(<Composer value="" onChange={vi.fn()} onSubmit={vi.fn()} error="Не удалось отправить." />);
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Не удалось отправить.");
    expect(input).toHaveAttribute("aria-describedby", alert.id);
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(container.querySelector(".composer__bar")?.previousElementSibling).toBe(alert);
  });
});

describe("Composer: плавающий вид (окно захвата)", () => {
  const floating = (props: Partial<React.ComponentProps<typeof Composer>> = {}) =>
    setup({ floating: true, buttons: {}, ...props });

  it("область alert есть всегда, пустая и под полем; поле не помечено недопустимым и ничем не описано", () => {
    const { input, container } = floating();
    const alert = screen.getByRole("alert");
    expect(alert).toBeEmptyDOMElement();
    expect(container.querySelector(".composer")).toHaveClass("composer--floating");
    expect(container.querySelector(".composer__bar")?.nextElementSibling).toContainElement(alert);
    expect(input).not.toHaveAttribute("aria-invalid");
    expect(input).not.toHaveAttribute("aria-describedby");
  });

  it("ошибка приходит в тот же элемент, поле описано ею", () => {
    const { rerender, input } = floating();
    const before = screen.getByRole("alert");
    rerender(<Composer value="" onChange={vi.fn()} onSubmit={vi.fn()} floating buttons={{}} error="Не удалось отправить." />);
    expect(screen.getByRole("alert")).toBe(before);
    expect(before).toHaveTextContent("Не удалось отправить.");
    expect(input).toHaveAttribute("aria-describedby", before.id);
    expect(input).toHaveAttribute("aria-invalid", "true");
  });

  it("подсказка под полем описывает его; пока есть ошибка, подсказки нет, потом она возвращается", () => {
    const { rerender, input } = floating({ hint: "⌘Enter — дописать" });
    expect(screen.getByText("⌘Enter — дописать")).toHaveClass("composer__hint");
    expect(input).toHaveAccessibleDescription("⌘Enter — дописать");
    rerender(<Composer value="" onChange={vi.fn()} floating buttons={{}} hint="⌘Enter — дописать" error="Ошибка." />);
    expect(screen.queryByText("⌘Enter — дописать")).toBeNull();
    expect(input).toHaveAccessibleDescription("Ошибка.");
    rerender(<Composer value="" onChange={vi.fn()} floating buttons={{}} hint="⌘Enter — дописать" />);
    expect(input).toHaveAccessibleDescription("⌘Enter — дописать");
  });

  it("подсказка без floating не показывается (в главном окне её нет)", () => {
    setup({ hint: "⌘Enter — дописать" });
    expect(screen.queryByText("⌘Enter — дописать")).toBeNull();
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
