import { describe, expect, it, vi } from "vitest";
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

  it("за сегодня записей нет: списка нет, одна тихая строка", () => {
    const { container } = setup({ context: [] });
    expect(screen.queryByRole("list")).toBeNull();
    expect(screen.getByText("Сегодня записей ещё нет")).toBeInTheDocument();
    expect(container.querySelectorAll("article")).toHaveLength(0);
  });

  it("английский: подписи и подсказка", () => {
    setup({ appendTargetId: "c" }, "en");
    expect(screen.getByRole("group", { name: "Quick note" })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Latest entries today" })).toBeInTheDocument();
    expect(screen.getByText("⌘Enter — append")).toBeInTheDocument();
  });

  it("английский: пустое состояние", () => {
    setup({ context: [] }, "en");
    expect(screen.getByText("No entries yet today")).toBeInTheDocument();
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
  it("есть цель: подсказка стоит у нужной плашки и описывает поле для диктора", () => {
    const { field, container } = setup({ appendTargetId: "c" });
    const hint = screen.getByText("⌘Enter — дописать");
    expect(hint.closest("li")).toBe(container.querySelectorAll(".capture-context__item")[2]);
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

  it("место не прыгает: строка под плашкой есть у каждой записи и при подсказке, и без неё", () => {
    const { container, rerender } = setup({ appendTargetId: null });
    const hintRows = () => container.querySelectorAll(".capture-entry__hint");
    expect(hintRows()).toHaveLength(3);
    rerender({ appendTargetId: "b" });
    expect(hintRows()).toHaveLength(3);
    expect(container.querySelectorAll(".capture-context__item")).toHaveLength(3);
  });
});

describe("CapturePanel: ошибка", () => {
  it("место под ошибку есть всегда (пустая область alert), текст приходит в тот же элемент; текст в поле остаётся", () => {
    const { rerender, field } = setup({ value: "черновик" });
    const alert = screen.getByRole("alert");
    expect(alert).toBeEmptyDOMElement();
    rerender({ value: "черновик", error: "Не удалось сохранить запись. Текст остался в поле." });
    expect(screen.getByRole("alert")).toBe(alert);
    expect(alert).toHaveTextContent("Не удалось сохранить запись. Текст остался в поле.");
    expect(field).toHaveValue("черновик");
    expect(field).toHaveAttribute("aria-invalid", "true");
  });

  it("при ошибке и подсказке поле описано обеими", () => {
    const { field } = setup({ appendTargetId: "c", error: "Ошибка." });
    expect(field).toHaveAccessibleDescription("Ошибка. ⌘Enter — дописать");
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
