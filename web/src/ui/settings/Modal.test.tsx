import { useRef, useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { Modal } from "./Modal";

function Harness({ onClose = () => {} }: { onClose?: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button onClick={() => setOpen(true)}>Открыть</button>
      {open ? (
        <Modal
          title="Окно"
          closeLabel="Закрыть окно"
          onClose={() => {
            onClose();
            setOpen(false);
          }}
        >
          <button>Первая</button>
          <button>Вторая</button>
        </Modal>
      ) : null}
    </>
  );
}

const tab = (shift = false) => fireEvent.keyDown(document.activeElement ?? document.body, { key: "Tab", shiftKey: shift });

describe("Modal", () => {
  it("диалог: role, aria-modal, имя из заголовка", () => {
    render(
      <Modal title="Окно" closeLabel="Закрыть окно" onClose={() => {}}>
        <p>Текст</p>
      </Modal>,
    );
    const dialog = screen.getByRole("dialog", { name: "Окно" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(screen.getByRole("heading", { name: "Окно", level: 2 })).toBeInTheDocument();
  });

  it("при открытии фокус на первом управляющем элементе — кнопке закрытия", () => {
    render(
      <Modal title="Окно" closeLabel="Закрыть окно" onClose={() => {}}>
        <button>Первая</button>
      </Modal>,
    );
    expect(screen.getByRole("button", { name: "Закрыть окно" })).toHaveFocus();
  });

  it("Esc закрывает", () => {
    const onClose = vi.fn();
    render(
      <Modal title="Окно" closeLabel="Закрыть окно" onClose={onClose}>
        <button>Первая</button>
      </Modal>,
    );
    fireEvent.keyDown(screen.getByRole("button", { name: "Первая" }), { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("щелчок по подложке закрывает, по панели — нет", () => {
    const onClose = vi.fn();
    const { container } = render(
      <Modal title="Окно" closeLabel="Закрыть окно" onClose={onClose}>
        <p>Текст</p>
      </Modal>,
    );
    fireEvent.click(screen.getByText("Текст"));
    fireEvent.click(screen.getByRole("dialog"));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(container.querySelector(".modal__scrim")!);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("кнопка закрытия вызывает onClose", () => {
    const onClose = vi.fn();
    render(
      <Modal title="Окно" closeLabel="Закрыть окно" onClose={onClose}>
        <p>Текст</p>
      </Modal>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Закрыть окно" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("ловушка фокуса: Tab и Shift+Tab ходят по кругу внутри окна", () => {
    render(
      <>
        <button>Снаружи</button>
        <Modal title="Окно" closeLabel="Закрыть окно" onClose={() => {}}>
          <button>Первая</button>
          <button>Вторая</button>
        </Modal>
      </>,
    );
    const names = () => (document.activeElement as HTMLElement).textContent || (document.activeElement as HTMLElement).getAttribute("aria-label");
    expect(screen.getByRole("button", { name: "Закрыть окно" })).toHaveFocus();
    tab();
    expect(names()).toBe("Первая");
    tab();
    expect(names()).toBe("Вторая");
    tab();
    expect(screen.getByRole("button", { name: "Закрыть окно" })).toHaveFocus();
    tab(true);
    expect(names()).toBe("Вторая");
    expect(screen.getByRole("button", { name: "Снаружи" })).not.toHaveFocus();
  });

  it("фокус, ушедший за окно, возвращается внутрь", () => {
    render(
      <>
        <button>Снаружи</button>
        <Modal title="Окно" closeLabel="Закрыть окно" onClose={() => {}}>
          <button>Первая</button>
        </Modal>
      </>,
    );
    screen.getByRole("button", { name: "Снаружи" }).focus();
    expect(screen.getByRole("button", { name: "Закрыть окно" })).toHaveFocus();
  });

  it("при закрытии фокус возвращается на открывший элемент", () => {
    render(<Harness />);
    const opener = screen.getByRole("button", { name: "Открыть" });
    opener.focus();
    fireEvent.click(opener);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(opener).not.toHaveFocus();
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(opener).toHaveFocus();
  });

  it("returnFocusRef имеет приоритет над запомненным элементом", () => {
    function WithRef() {
      const target = useRef<HTMLButtonElement>(null);
      const [open, setOpen] = useState(true);
      return (
        <>
          <button ref={target}>Цель</button>
          {open ? (
            <Modal title="Окно" closeLabel="Закрыть окно" onClose={() => setOpen(false)} returnFocusRef={target}>
              <p>Текст</p>
            </Modal>
          ) : null}
        </>
      );
    }
    render(<WithRef />);
    fireEvent.click(screen.getByRole("button", { name: "Закрыть окно" }));
    expect(screen.getByRole("button", { name: "Цель" })).toHaveFocus();
  });

  it("окно над окном: клавиатуру и фокус ведёт только верхнее, Esc закрывает верхнее", () => {
    const closeUnder = vi.fn();
    const closeOver = vi.fn();
    render(
      <>
        <Modal title="Нижнее" closeLabel="Закрыть нижнее" onClose={closeUnder}>
          <button>Нижняя кнопка</button>
        </Modal>
        <Modal title="Верхнее" closeLabel="Закрыть верхнее" onClose={closeOver}>
          <button>Верхняя кнопка</button>
        </Modal>
      </>,
    );
    const over = screen.getByRole("dialog", { name: "Верхнее" });
    expect(over.contains(document.activeElement)).toBe(true);
    // Tab ходит по кругу внутри верхнего и не уходит в нижнее.
    tab();
    tab();
    tab();
    expect(over.contains(document.activeElement)).toBe(true);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(closeOver).toHaveBeenCalledTimes(1);
    expect(closeUnder).not.toHaveBeenCalled();
  });

  it("если в окне одна кнопка (закрытие), Tab остаётся на ней", () => {
    render(
      <Modal title="Окно" closeLabel="Закрыть окно" onClose={() => {}}>
        <p>Текст</p>
      </Modal>,
    );
    // Кнопка закрытия есть всегда: Tab не уходит из окна, а остаётся на ней.
    tab();
    expect(screen.getByRole("button", { name: "Закрыть окно" })).toHaveFocus();
  });
});
