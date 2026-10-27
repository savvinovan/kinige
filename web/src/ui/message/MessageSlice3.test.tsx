import { act, fireEvent, render, screen, within } from "../../test/render";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Message, type MessageProps } from "./Message";

const sentAt = new Date(2026, 9, 20, 9, 12);
const view = (props: Partial<MessageProps> = {}) => <Message markdown="Купить хлеб" sentAt={sentAt} {...props} />;
const bar = () => screen.getByRole("toolbar", { name: "Действия с сообщением" });
const menu = () => screen.queryByRole("menu", { name: "Меню записи" });
const item = (name: string) => within(screen.getByRole("menu")).getByRole("menuitem", { name });
const article = (container: HTMLElement) => container.querySelector("article") as HTMLElement;

const stubClipboard = (writeText: (text: string) => Promise<void>) => {
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
};

afterEach(() => vi.restoreAllMocks());

describe("ряд действий оживает", () => {
  it("«Редактировать», «Сделать задачей» и «В избранное» вызывают обработчики; переключатели присылают новое состояние", () => {
    const onEdit = vi.fn();
    const onToggleTask = vi.fn();
    const onToggleFavorite = vi.fn();
    render(view({ onEdit, onToggleTask, onToggleFavorite }));
    fireEvent.click(within(bar()).getByRole("button", { name: "Редактировать" }));
    fireEvent.click(within(bar()).getByRole("button", { name: "Сделать задачей" }));
    fireEvent.click(within(bar()).getByRole("button", { name: "В избранное" }));
    expect(onEdit).toHaveBeenCalledTimes(1);
    expect(onToggleTask).toHaveBeenCalledExactlyOnceWith(true);
    expect(onToggleFavorite).toHaveBeenCalledExactlyOnceWith(true);
    expect(within(bar()).getByRole("button", { name: "Редактировать" })).not.toHaveAttribute("aria-disabled");
  });

  it("задача и избранное включены: подписи для диктора меняются, значки в активном виде, щелчок выключает", () => {
    const onToggleTask = vi.fn();
    const onToggleFavorite = vi.fn();
    render(view({ isTask: true, favorite: true, onToggleTask, onToggleFavorite }));
    const task = within(bar()).getByRole("button", { name: "Убрать задачу" });
    const favorite = within(bar()).getByRole("button", { name: "Убрать из избранного" });
    expect(task).toHaveClass("message-actions__on");
    expect(favorite).toHaveClass("message-actions__on", "message-actions__filled");
    fireEvent.click(task);
    fireEvent.click(favorite);
    expect(onToggleTask).toHaveBeenCalledWith(false);
    expect(onToggleFavorite).toHaveBeenCalledWith(false);
  });

  it("запись только для чтения: всё, кроме «Копировать» и «Ещё», недоступно с пояснением и ничего не вызывает", () => {
    const onEdit = vi.fn();
    render(view({ readOnly: true, onEdit, onToggleTask: vi.fn(), onToggleFavorite: vi.fn() }));
    for (const name of ["Редактировать", "Сделать задачей", "В избранное"]) {
      const button = within(bar()).getByRole("button", { name });
      expect(button).toHaveAttribute("aria-disabled", "true");
      expect(button).toHaveAttribute("title", expect.stringContaining("только читать"));
    }
    fireEvent.click(within(bar()).getByRole("button", { name: "Редактировать" }));
    expect(onEdit).not.toHaveBeenCalled();
    expect(within(bar()).getByRole("button", { name: "Копировать" })).not.toHaveAttribute("aria-disabled");
    expect(within(bar()).getByRole("button", { name: "Ещё" })).not.toHaveAttribute("aria-disabled");
  });
});

describe("контекстное меню записи", () => {
  const handlers = () => ({
    onEdit: vi.fn(),
    onToggleTask: vi.fn(),
    onToggleFavorite: vi.fn(),
    onChangeDate: vi.fn(),
    onDelete: vi.fn(),
  });

  it("правый щелчок по плашке открывает меню из семи пунктов, «Удалить» — опасное и за чертой", () => {
    const { container } = render(view(handlers()));
    expect(menu()).toBeNull();
    expect(fireEvent.contextMenu(article(container), { clientX: 40, clientY: 30 })).toBe(false);
    expect(menu()).toBeInTheDocument();
    const items = within(screen.getByRole("menu")).getAllByRole("menuitem");
    expect(items.map((i) => i.textContent)).toEqual([
      "Копировать",
      "Редактировать",
      "Сделать задачей",
      "В избранное",
      "Изменить дату…",
      "Переместить в чат",
      "Удалить",
    ]);
    expect(items[6]).toHaveClass("context-menu__item--danger");
    expect(items[6]?.parentElement).toHaveClass("context-menu__group");
    // Без списка чатов и обработчика переноса пункт виден, но недоступен.
    expect(items[5]).toHaveAttribute("aria-disabled", "true");
    // Первый пункт получает фокус: меню сразу работает с клавиатуры.
    expect(items[0]).toHaveFocus();
  });

  it("кнопка «Ещё» открывает то же меню; ряд действий остаётся видимым, пока меню открыто", () => {
    const { container } = render(view(handlers()));
    fireEvent.click(within(bar()).getByRole("button", { name: "Ещё" }));
    expect(menu()).toBeInTheDocument();
    expect(container.querySelector(".message-wrap")).toHaveClass("message-wrap--actions");
    expect(within(bar()).getByRole("button", { name: "Ещё" })).toHaveAttribute("aria-expanded", "true");
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    expect(menu()).toBeNull();
    expect(container.querySelector(".message-wrap")).not.toHaveClass("message-wrap--actions");
  });

  it("Shift+F10 и клавиша меню на кнопке ряда открывают меню; на обычном тексте и в других режимах — нет", () => {
    const { container, rerender } = render(view(handlers()));
    const edit = within(bar()).getByRole("button", { name: "Редактировать" });
    edit.focus();
    fireEvent.keyDown(edit, { key: "F10", shiftKey: true });
    expect(menu()).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    fireEvent.keyDown(edit, { key: "ContextMenu" });
    expect(menu()).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    rerender(view({ ...handlers(), mode: "deleted" }));
    fireEvent.contextMenu(article(container));
    expect(menu()).toBeNull();
  });

  it("выбор пункта закрывает меню и вызывает обработчик; переключатели присылают новое состояние", () => {
    const h = handlers();
    const { container } = render(view({ ...h, isTask: true, favorite: false }));
    const open = () => fireEvent.contextMenu(article(container));
    open();
    fireEvent.click(item("Редактировать"));
    expect(h.onEdit).toHaveBeenCalledTimes(1);
    expect(menu()).toBeNull();
    open();
    fireEvent.click(item("Убрать задачу"));
    expect(h.onToggleTask).toHaveBeenCalledWith(false);
    open();
    fireEvent.click(item("В избранное"));
    expect(h.onToggleFavorite).toHaveBeenCalledWith(true);
    open();
    fireEvent.click(item("Изменить дату…"));
    expect(h.onChangeDate).toHaveBeenCalledTimes(1);
    open();
    fireEvent.click(item("Удалить"));
    expect(h.onDelete).toHaveBeenCalledTimes(1);
  });

  it("запись только для чтения: «Копировать» доступно, остальные пункты недоступны и не вызываются", () => {
    const h = handlers();
    const { container } = render(view({ ...h, readOnly: true }));
    fireEvent.contextMenu(article(container));
    expect(item("Копировать")).not.toHaveAttribute("aria-disabled");
    for (const name of ["Редактировать", "Сделать задачей", "В избранное", "Изменить дату…", "Удалить"]) {
      expect(item(name)).toHaveAttribute("aria-disabled", "true");
    }
    fireEvent.click(item("Удалить"));
    expect(h.onDelete).not.toHaveBeenCalled();
    expect(menu()).toBeInTheDocument();
  });

  it("без обработчиков пункты видны, но недоступны (D1), «Копировать» работает", () => {
    const { container } = render(view());
    fireEvent.contextMenu(article(container));
    expect(item("Редактировать")).toHaveAttribute("aria-disabled", "true");
    expect(item("Копировать")).not.toHaveAttribute("aria-disabled");
  });

  it("правый щелчок внутри меню не открывает второе меню", () => {
    const { container } = render(view(handlers()));
    fireEvent.contextMenu(article(container), { clientX: 10, clientY: 10 });
    fireEvent.contextMenu(item("Копировать"));
    expect(screen.getAllByRole("menu")).toHaveLength(1);
  });
});

describe("«Копировать»: выделение или весь Markdown", () => {
  const select = (element: Element) => {
    const selection = window.getSelection() as Selection;
    const range = document.createRange();
    range.selectNodeContents(element);
    selection.removeAllRanges();
    selection.addRange(range);
  };
  afterEach(() => window.getSelection()?.removeAllRanges());

  it("без выделения кнопка копирует весь Markdown, с выделением внутри плашки — только выделенное", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    stubClipboard(writeText);
    render(view({ markdown: "**жирный** хлеб" }));
    await act(async () => {
      fireEvent.click(within(bar()).getByRole("button", { name: "Копировать" }));
    });
    expect(writeText).toHaveBeenLastCalledWith("**жирный** хлеб");
    select(screen.getByText("жирный"));
    await act(async () => {
      fireEvent.click(within(bar()).getByRole("button", { name: /Копировать|Скопировано/ }));
    });
    expect(writeText).toHaveBeenLastCalledWith("жирный");
  });

  it("выделение вне плашки не в счёт: копируется весь Markdown", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    stubClipboard(writeText);
    render(
      <div>
        <p>Снаружи</p>
        {view({ markdown: "внутри" })}
      </div>,
    );
    select(screen.getByText("Снаружи"));
    await act(async () => {
      fireEvent.click(within(bar()).getByRole("button", { name: "Копировать" }));
    });
    expect(writeText).toHaveBeenCalledWith("внутри");
  });

  it("пункт меню копирует выделение, запомненное в момент открытия меню", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    stubClipboard(writeText);
    const { container } = render(view({ markdown: "один два три" }));
    select(screen.getByText("один два три"));
    fireEvent.contextMenu(article(container));
    window.getSelection()?.removeAllRanges();
    await act(async () => {
      fireEvent.click(item("Копировать"));
    });
    expect(writeText).toHaveBeenCalledWith("один два три");
  });
});

