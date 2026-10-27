import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { demoEntry } from "../../api/demoData";
import { fireEvent, render, screen } from "../../test/render";
import { CapturePanel, type CapturePanelProps } from "./CapturePanel";

const TODAY = "2026-10-20";
const entries = [
  demoEntry("a", TODAY, "09:12", "Первая мысль"),
  demoEntry("b", TODAY, "11:30", "### План\n- [x] сделано\n- [ ] ждёт"),
  demoEntry("c", TODAY, "13:45", "Прочитал [статью](https://example.com/a) и записал **вывод**"),
];

function setup(props: Partial<CapturePanelProps> = {}, language: "ru" | "en" = "ru") {
  const handlers = { onChange: vi.fn(), onSend: vi.fn(), onAppend: vi.fn(), onHide: vi.fn() };
  const all: CapturePanelProps = { context: entries, appendTargetId: null, value: "", ...handlers, ...props };
  const view = render(<CapturePanel {...all} />, { language });
  const rerender = (next: Partial<CapturePanelProps>) => view.rerender(<CapturePanel {...all} {...next} />);
  return { ...handlers, ...view, rerender, field: screen.getByRole("textbox", { name: /Новая заметка|New note/ }) as HTMLTextAreaElement };
}

describe("CapturePanel: вид", () => {
  it("окно с подписью, список контекста, время у каждой записи в поясе записи, Markdown свёрстан", () => {
    const { container } = setup();
    expect(screen.getByRole("group", { name: "Быстрая заметка" })).toBeInTheDocument();
    const list = screen.getByRole("list", { name: "Последние записи за сегодня" });
    expect(list.querySelectorAll(":scope > li")).toHaveLength(3);
    expect([...container.querySelectorAll("time")].map((time) => time.textContent)).toEqual(["09:12", "11:30", "13:45"]);
    expect(container.querySelector("h3")?.textContent).toBe("План");
    expect(container.querySelectorAll("li.task")).toHaveLength(2);
    expect(container.querySelector("strong")?.textContent).toBe("вывод");
  });

  it("самая новая запись последняя, ближе к полю: порядок как в переданном массиве", () => {
    setup();
    const texts = screen.getAllByRole("article").map((article) => article.textContent);
    expect(texts[0]).toContain("Первая мысль");
    expect(texts[2]).toContain("Прочитал");
  });

  it("поле как в главном окне, но без кнопок «файл», «эмодзи» и «микрофон»; есть автофокус", () => {
    const { field } = setup();
    expect(field).toHaveFocus();
    expect(screen.queryByRole("button", { name: "Прикрепить файл" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Эмодзи" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Записать голосовое" })).toBeNull();
  });

  it("за сегодня записей нет: только поле, ни списка, ни пояснений", () => {
    const { container, field } = setup({ context: [] });
    expect(screen.queryByRole("list")).toBeNull();
    expect(container.querySelectorAll("article")).toHaveLength(0);
    expect(container.querySelector(".capture-context")).toBeNull();
    expect(field).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Быстрая заметка" })).toHaveTextContent("");
  });

  it("английский: подписи и подсказка", () => {
    setup({ appendTargetId: "c" }, "en");
    expect(screen.getByRole("group", { name: "Quick note" })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Latest entries today" })).toBeInTheDocument();
    expect(screen.getByText("⌘Enter — append")).toBeInTheDocument();
  });

  it("английский: без записей только поле с подсказкой «Write a note…»", () => {
    const { field } = setup({ context: [] }, "en");
    expect(field).toHaveAttribute("placeholder", "Write a note…");
  });
});

describe("CapturePanel: ссылки не открываются", () => {
  it("в контексте нет ни <a>, ни адреса, ни ничего, что берёт фокус; щелчок по тексту ссылки ничего не вызывает", () => {
    const { container } = setup();
    expect(container.querySelector("a, [href]")).toBeNull();
    expect(container.querySelector("[tabindex]")).toBeNull();
    const link = container.querySelector(".capture-link");
    expect(link).toHaveTextContent("статью");
    fireEvent.click(link!);
    expect(container.querySelectorAll("button")).toHaveLength(0);
  });

  it("блок кода в контексте не берёт фокус с клавиатуры", () => {
    const { container } = setup({ context: [demoEntry("x", TODAY, "10:00", "```\nlet a = 1;\n```")] });
    expect(container.querySelector("pre")).not.toBeNull();
    expect(container.querySelector("pre[tabindex]")).toBeNull();
  });
});

describe("CapturePanel: подсказка «⌘Enter — дописать»", () => {
  it("есть цель: подсказка стоит под полем и описывает поле для диктора", () => {
    const { field, container } = setup({ appendTargetId: "c" });
    const hint = screen.getByText("⌘Enter — дописать");
    expect(hint).toHaveClass("composer__hint");
    expect(container.querySelector(".composer__bar")?.nextElementSibling).toContainElement(hint);
    expect(field).toHaveAccessibleDescription("⌘Enter — дописать");
  });

  it("нет цели (null): подсказки нет и поле ничем не описано", () => {
    const { field } = setup({ appendTargetId: null });
    expect(screen.queryByText(/⌘Enter/)).toBeNull();
    expect(field).not.toHaveAttribute("aria-describedby");
  });

  it("цель, которой нет в контексте, подсказку не показывает", () => {
    setup({ appendTargetId: "нет-такой" });
    expect(screen.queryByText(/⌘Enter/)).toBeNull();
  });

  it("поле не прыгает: плашки не зависят от подсказки, а подсказка стоит ПОД полем (окно растёт вниз)", () => {
    const { container, rerender } = setup({ appendTargetId: null });
    const before = container.querySelector(".capture-context")?.innerHTML;
    expect(container.querySelectorAll(".composer__hint")).toHaveLength(0);
    rerender({ appendTargetId: "b" });
    expect(container.querySelector(".capture-context")?.innerHTML).toBe(before);
    expect(container.querySelectorAll(".composer__hint")).toHaveLength(1);
    expect(container.querySelectorAll(".capture-context__item")).toHaveLength(3);
    const bar = container.querySelector(".composer__bar");
    expect(bar?.compareDocumentPosition(container.querySelector(".composer__hint")!)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
  });
});

describe("CapturePanel: ошибка", () => {
  it("область alert есть всегда (пустая, без высоты), текст приходит в тот же элемент и стоит под полем; текст в поле остаётся", () => {
    const { rerender, field } = setup({ value: "черновик" });
    const alert = screen.getByRole("alert");
    expect(alert).toBeEmptyDOMElement();
    rerender({ value: "черновик", error: "Не удалось сохранить запись. Текст остался в поле." });
    expect(screen.getByRole("alert")).toBe(alert);
    expect(alert).toHaveTextContent("Не удалось сохранить запись. Текст остался в поле.");
    expect(field).toHaveValue("черновик");
    expect(field).toHaveAttribute("aria-invalid", "true");
  });

  it("при ошибке подсказки нет (одна строка под полем): поле описано ошибкой, подсказка возвращается после неё", () => {
    const { field, rerender } = setup({ appendTargetId: "c", error: "Ошибка." });
    expect(field).toHaveAccessibleDescription("Ошибка.");
    expect(screen.queryByText("⌘Enter — дописать")).toBeNull();
    rerender({ appendTargetId: "c", error: undefined });
    expect(field).toHaveAccessibleDescription("⌘Enter — дописать");
  });
});

describe("CapturePanel: клавиши", () => {
  it("Enter — onSend, ⌘Enter и Ctrl+Enter — onAppend", () => {
    const { field, onSend, onAppend } = setup({ value: "и ещё" });
    fireEvent.keyDown(field, { key: "Enter" });
    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onAppend).not.toHaveBeenCalled();
    fireEvent.keyDown(field, { key: "Enter", metaKey: true });
    fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    expect(onAppend).toHaveBeenCalledTimes(2);
    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("кнопка «Отправить» при тексте — onSend", () => {
    const { onSend } = setup({ value: "текст" });
    fireEvent.click(screen.getByRole("button", { name: "Отправить" }));
    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("ввод уходит наверх", () => {
    const { field, onChange } = setup();
    fireEvent.change(field, { target: { value: "привет" } });
    expect(onChange).toHaveBeenCalledWith("привет");
  });

  it("Esc в поле скрывает окно", () => {
    const { field, onHide } = setup({ value: "черновик" });
    fireEvent.keyDown(field, { key: "Escape" });
    expect(onHide).toHaveBeenCalledTimes(1);
  });

  it("Esc скрывает окно и вне поля (фокус на теле страницы)", () => {
    const { onHide } = setup();
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(onHide).toHaveBeenCalledTimes(1);
  });

  it("Esc во время ввода через IME не скрывает окно", () => {
    const { field, onHide } = setup();
    fireEvent.keyDown(field, { key: "Escape", isComposing: true });
    expect(onHide).not.toHaveBeenCalled();
  });

  it("после размонтирования Esc ничего не вызывает", () => {
    const { onHide, unmount } = setup();
    unmount();
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(onHide).not.toHaveBeenCalled();
  });

  it("окно получило фокус (показано снова): курсор возвращается в поле", () => {
    const { field } = setup();
    field.blur();
    expect(field).not.toHaveFocus();
    fireEvent.focus(window);
    expect(field).toHaveFocus();
  });

  it("ни тостов, ни подтверждений: кнопок нет, пока поле пустое", () => {
    setup();
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("CapturePanel: высота окна", () => {
  const callbacks: (() => void)[] = [];
  let height = 150.3;
  beforeEach(() => {
    callbacks.length = 0;
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(callback: () => void) {
          callbacks.push(callback);
        }
        observe() {}
        disconnect() {}
      },
    );
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(() => ({ height }) as DOMRect);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("высота содержимого (вверх до целого, вместе с полем под тень) уходит при показе и при каждом изменении, повторов нет", () => {
    const onHeight = vi.fn();
    setup({ onHeight });
    expect(onHeight).toHaveBeenCalledTimes(1);
    expect(onHeight).toHaveBeenLastCalledWith(151);
    callbacks.forEach((callback) => callback());
    expect(onHeight).toHaveBeenCalledTimes(1);
    height = 214;
    callbacks.forEach((callback) => callback());
    expect(onHeight).toHaveBeenCalledTimes(2);
    expect(onHeight).toHaveBeenLastCalledWith(214);
  });

  it("нулевая высота (страница ещё не разложена) не отправляется", () => {
    height = 0;
    const onHeight = vi.fn();
    setup({ onHeight });
    expect(onHeight).not.toHaveBeenCalled();
    height = 150.3;
  });
});
