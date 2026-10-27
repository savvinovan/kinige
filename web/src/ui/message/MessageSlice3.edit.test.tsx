import { fireEvent, render, screen } from "../../test/render";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Message, type MessageProps } from "./Message";

const sentAt = new Date(2026, 9, 20, 9, 12);
const view = (props: Partial<MessageProps> = {}) => <Message markdown="Купить хлеб" sentAt={sentAt} {...props} />;
const bar = () => screen.getByRole("toolbar", { name: "Действия с сообщением" });
const menu = () => screen.queryByRole("menu", { name: "Меню записи" });
const article = (container: HTMLElement) => container.querySelector("article") as HTMLElement;

afterEach(() => vi.restoreAllMocks());

describe("правка в плашке", () => {
  const editing = (props: Partial<MessageProps> = {}) =>
    view({ mode: "editing", markdown: "Первая строка\nвторая", onSaveEdit: vi.fn(), onCancelEdit: vi.fn(), ...props });
  const field = () => screen.getByRole("textbox", { name: "Правка записи" }) as HTMLTextAreaElement;

  it("вместо текста поле с исходным Markdown, фокус в нём, каретка в конце; ряда действий и меню нет, время на месте", () => {
    const { container } = render(editing());
    expect(field().value).toBe("Первая строка\nвторая");
    expect(field()).toHaveFocus();
    expect(field().selectionStart).toBe(field().value.length);
    expect(field().selectionEnd).toBe(field().value.length);
    expect(screen.queryByRole("toolbar")).toBeNull();
    fireEvent.contextMenu(article(container));
    expect(menu()).toBeNull();
    expect(screen.getByText("09:12")).toBeInTheDocument();
    expect(container.querySelector("article")).toHaveClass("message--editing");
  });

  it("подсказка про Enter и Esc — для диктора и в title, визуально отдельной строки нет", () => {
    render(editing());
    expect(field()).toHaveAccessibleDescription("Enter — сохранить, Esc — отмена");
    expect(field()).toHaveAttribute("title", "Enter — сохранить, Esc — отмена");
  });

  it("Enter сохраняет изменённый текст, Esc отменяет", () => {
    const onSaveEdit = vi.fn();
    const onCancelEdit = vi.fn();
    render(editing({ onSaveEdit, onCancelEdit }));
    fireEvent.change(field(), { target: { value: "Новый текст" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(onSaveEdit).toHaveBeenCalledExactlyOnceWith("Новый текст");
    fireEvent.keyDown(field(), { key: "Escape" });
    expect(onCancelEdit).toHaveBeenCalledTimes(1);
  });

  it("пустой и пробельный текст Enter игнорирует; Shift+Enter и Enter во время IME не сохраняют", () => {
    const onSaveEdit = vi.fn();
    render(editing({ onSaveEdit }));
    fireEvent.change(field(), { target: { value: " \n  " } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(onSaveEdit).not.toHaveBeenCalled();
    fireEvent.change(field(), { target: { value: "текст" } });
    expect(fireEvent.keyDown(field(), { key: "Enter", shiftKey: true })).toBe(true);
    fireEvent.keyDown(field(), { key: "Enter", isComposing: true });
    expect(onSaveEdit).not.toHaveBeenCalled();
    expect(fireEvent.keyDown(field(), { key: "Enter" })).toBe(false);
    expect(onSaveEdit).toHaveBeenCalledWith("текст");
  });

  it("Esc не всплывает дальше поля (закрыть диалог или сбросить фильтр должен не он)", () => {
    const outer = vi.fn();
    document.addEventListener("keydown", outer);
    render(editing());
    fireEvent.keyDown(field(), { key: "Escape" });
    document.removeEventListener("keydown", outer);
    expect(outer).not.toHaveBeenCalled();
  });

  it("ошибка сохранения — строка role=alert внутри плашки; без ошибки её нет", () => {
    const { container, rerender } = render(editing());
    expect(screen.queryByRole("alert")).toBeNull();
    rerender(editing({ editError: "Не удалось сохранить запись." }));
    expect(screen.getByRole("alert")).toHaveTextContent("Не удалось сохранить запись.");
    expect(container.querySelector("article")).toContainElement(screen.getByRole("alert"));
  });

  it("ширина плашки до правки закрепляется переменной и снимается при выходе из правки", () => {
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
      return this.tagName === "ARTICLE" ? 321 : 0;
    });
    const { container, rerender } = render(view());
    rerender(editing());
    // Ширина плашки в jsdom подменена (321); в переменную она попадает числом в пикселях.
    expect(article(container).style.getPropertyValue("--message-edit-width")).toBe(`${321}px`);
    rerender(view());
    expect(article(container).style.getPropertyValue("--message-edit-width")).toBe("");
  });

  it("после правки ряд действий возвращается и снова считает положение", () => {
    const { rerender } = render(editing());
    expect(screen.queryByRole("toolbar")).toBeNull();
    rerender(view());
    expect(bar()).toBeInTheDocument();
  });
});

describe("удалённая плашка", () => {
  it("текст на месте, вместо времени «Удалено · Вернуть»; ряда действий и меню нет", () => {
    const onRestore = vi.fn();
    const { container } = render(view({ mode: "deleted", onRestore, onDelete: vi.fn() }));
    expect(screen.getByText("Купить хлеб")).toBeInTheDocument();
    expect(container.querySelector("article")).toHaveClass("message--deleted");
    expect(container.querySelector(".message__deleted")).toHaveTextContent("Удалено · Вернуть");
    expect(screen.queryByText("09:12")).toBeNull();
    expect(screen.queryByRole("toolbar")).toBeNull();
    fireEvent.contextMenu(article(container));
    expect(menu()).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Вернуть удалённую запись" }));
    expect(onRestore).toHaveBeenCalledTimes(1);
  });

  it("«Вернуть» — настоящая кнопка с видимым текстом в имени (WCAG 2.5.3), доступна с клавиатуры", () => {
    render(view({ mode: "deleted" }));
    const restore = screen.getByRole("button", { name: /^Вернуть/ });
    expect(restore).toHaveTextContent("Вернуть");
    restore.focus();
    expect(restore).toHaveFocus();
  });

  it("на английском: «Deleted · Restore»", () => {
    const { container } = render(view({ mode: "deleted" }), { language: "en" });
    expect(container.querySelector(".message__deleted")).toHaveTextContent("Deleted · Restore");
  });

  it("флажок задачи у удалённой плашки не нажимается", () => {
    const onToggleDone = vi.fn();
    render(view({ mode: "deleted", isTask: true, onToggleDone }));
    const check = screen.getByRole("checkbox", { name: "Задача выполнена" });
    expect(check).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(check);
    expect(onToggleDone).not.toHaveBeenCalled();
  });
});

describe("задача и избранное", () => {
  it("обычная запись без флажка; задача — флажок role=checkbox с aria-checked и именем без состояния", () => {
    const { rerender, container } = render(view());
    expect(screen.queryByRole("checkbox")).toBeNull();
    rerender(view({ isTask: true }));
    const check = screen.getByRole("checkbox", { name: "Задача выполнена" });
    expect(check).toHaveAttribute("aria-checked", "false");
    expect(container.querySelector("article")).toHaveClass("message--task");
    rerender(view({ isTask: true, taskDone: true }));
    expect(screen.getByRole("checkbox", { name: "Задача выполнена" })).toHaveAttribute("aria-checked", "true");
    expect(container.querySelector("article")).toHaveClass("message--task-done");
  });

  it("щелчок по флажку присылает новое состояние; без обработчика и у записи только для чтения не работает", () => {
    const onToggleDone = vi.fn();
    const { rerender } = render(view({ isTask: true, onToggleDone }));
    fireEvent.click(screen.getByRole("checkbox"));
    expect(onToggleDone).toHaveBeenLastCalledWith(true);
    rerender(view({ isTask: true, taskDone: true, onToggleDone }));
    fireEvent.click(screen.getByRole("checkbox"));
    expect(onToggleDone).toHaveBeenLastCalledWith(false);
    onToggleDone.mockClear();
    rerender(view({ isTask: true, readOnly: true, onToggleDone }));
    fireEvent.click(screen.getByRole("checkbox"));
    expect(onToggleDone).not.toHaveBeenCalled();
    rerender(view({ isTask: true }));
    expect(screen.getByRole("checkbox")).toHaveAttribute("aria-disabled", "true");
  });

  it("флажок — настоящая кнопка: берёт фокус с клавиатуры", () => {
    render(view({ isTask: true, onToggleDone: vi.fn() }));
    const check = screen.getByRole("checkbox");
    expect(check.tagName).toBe("BUTTON");
    check.focus();
    expect(check).toHaveFocus();
  });

  it("избранное — тихая отметка со звездой и подписью для диктора рядом со временем", () => {
    const { container, rerender } = render(view());
    expect(screen.queryByRole("img", { name: "В избранном" })).toBeNull();
    rerender(view({ favorite: true }));
    const mark = screen.getByRole("img", { name: "В избранном" });
    expect(container.querySelector(".message__meta")).toContainElement(mark);
    expect(container.querySelector("article")).toHaveClass("message--favorite");
    expect(screen.getByText("09:12")).toBeInTheDocument();
  });
});

describe("теги и квадраты в тексте", () => {
  it("тег из списка подсвечен и нажимается мышью, Enter и пробелом; присылается в нижнем регистре без #", () => {
    const onTagClick = vi.fn();
    render(view({ markdown: "Мысль про #Идея и #другое", tags: ["идея"], onTagClick }));
    const tag = screen.getByRole("button", { name: "#Идея" });
    expect(tag).toHaveClass("tag");
    expect(tag).toHaveAttribute("tabindex", "0");
    expect(screen.queryByRole("button", { name: "#другое" })).toBeNull();
    fireEvent.click(tag);
    expect(onTagClick).toHaveBeenLastCalledWith("идея");
    expect(fireEvent.keyDown(tag, { key: "Enter" })).toBe(false);
    expect(onTagClick).toHaveBeenCalledTimes(2);
    fireEvent.keyDown(tag, { key: " " });
    expect(onTagClick).toHaveBeenCalledTimes(3);
    fireEvent.keyDown(tag, { key: "a" });
    expect(onTagClick).toHaveBeenCalledTimes(3);
  });

  it("без обработчика щелчок по тегу ничего не ломает", () => {
    render(view({ markdown: "#идея", tags: ["идея"] }));
    expect(() => fireEvent.click(screen.getByText("#идея"))).not.toThrow();
  });

  it("квадраты «- [ ]» становятся флажками с порядковым номером; щелчок и пробел присылают номер и новое состояние", () => {
    const onToggleTaskItem = vi.fn();
    render(view({ markdown: "- [x] Поход\n- [ ] Фотографии\n- [ ] Книга", onToggleTaskItem }));
    const boxes = screen.getAllByRole("checkbox", { name: "Пункт выполнен" });
    expect(boxes.map((box) => box.getAttribute("aria-checked"))).toEqual(["true", "false", "false"]);
    expect(boxes[0]).toHaveAttribute("tabindex", "0");
    fireEvent.click(boxes[1] as HTMLElement);
    expect(onToggleTaskItem).toHaveBeenLastCalledWith(1, true);
    fireEvent.click(boxes[0] as HTMLElement);
    expect(onToggleTaskItem).toHaveBeenLastCalledWith(0, false);
    expect(fireEvent.keyDown(boxes[2] as HTMLElement, { key: " " })).toBe(false);
    expect(onToggleTaskItem).toHaveBeenLastCalledWith(2, true);
  });

  it("без обработчика, у записи только для чтения и в удалённой плашке квадраты остаются картинками (D1)", () => {
    const text = "- [x] Поход\n- [ ] Фотографии";
    const { rerender } = render(view({ markdown: text }));
    expect(screen.getAllByRole("img")).toHaveLength(2);
    rerender(view({ markdown: text, onToggleTaskItem: vi.fn(), readOnly: true }));
    expect(screen.queryByRole("checkbox")).toBeNull();
    rerender(view({ markdown: text, onToggleTaskItem: vi.fn(), mode: "deleted" }));
    expect(screen.queryByRole("checkbox")).toBeNull();
  });

  it("обычные ссылки в той же плашке по-прежнему уходят в onOpenLink", () => {
    const onOpenLink = vi.fn();
    render(view({ markdown: "[сайт](https://example.com) #идея", tags: ["идея"], onOpenLink, onTagClick: vi.fn() }));
    fireEvent.click(screen.getByRole("link", { name: "сайт" }));
    expect(onOpenLink).toHaveBeenCalledWith("https://example.com");
  });
});
