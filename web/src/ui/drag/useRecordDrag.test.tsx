import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { rect } from "../../test/rect";
import { fireEvent, render, screen } from "../../test/render";
import { DROP_ATTRIBUTE, useRecordDrag } from "./useRecordDrag";

// jsdom не раскладывает элементы и не умеет `elementFromPoint`: границы плашки и «элемент под указателем» подставляются.
const PLATE = rect(100, 100, 200, 40);

interface HarnessProps {
  canDrop?: (chatId: string) => boolean;
  onDrop: (messageId: string, chatId: string) => void;
}

function Harness({ canDrop = () => true, onDrop }: HarnessProps) {
  const drag = useRecordDrag({ canDrop, onDrop });
  return (
    <div>
      <article
        data-testid="plate"
        className="message"
        ref={(element) => {
          if (element) element.getBoundingClientRect = () => PLATE;
        }}
        onPointerDown={(event) => drag.begin("m1")(event, event.currentTarget)}
      >
        Текст записи
        <button type="button">кнопка в плашке</button>
      </article>
      <button type="button" {...{ [DROP_ATTRIBUTE]: "books" }}>
        Книги
      </button>
      <button type="button" {...{ [DROP_ATTRIBUTE]: "ideas" }}>
        Идеи
      </button>
      <output data-testid="over">{drag.overChatId ?? "-"}</output>
      {drag.ghost}
    </div>
  );
}

let under: Element | null = null;
const original = document.elementFromPoint;

beforeEach(() => {
  under = null;
  document.elementFromPoint = () => under;
});
afterEach(() => {
  document.elementFromPoint = original;
  document.body.classList.remove("is-record-dragging");
});

const plate = () => screen.getByTestId("plate");
const press = (target: Element = plate(), init: PointerEventInit = {}) =>
  fireEvent.pointerDown(target, { button: 0, pointerId: 1, pointerType: "mouse", clientX: 150, clientY: 120, ...init });
const move = (x: number, y: number) => fireEvent.pointerMove(document, { pointerId: 1, clientX: x, clientY: y });
const release = (x: number, y: number) => fireEvent.pointerUp(document, { pointerId: 1, clientX: x, clientY: y });

describe("перетаскивание плашки на чат", () => {
  it("выход за границы плашки начинает перетаскивание: «призрак», подсветка цели, бросок вызывает onDrop", () => {
    const onDrop = vi.fn();
    render(<Harness onDrop={onDrop} />);
    press();
    // Внутри плашки это ещё выделение текста: ни призрака, ни класса на странице.
    move(160, 125);
    expect(document.querySelector(".record-ghost")).toBeNull();
    expect(document.body).not.toHaveClass("is-record-dragging");

    under = screen.getByRole("button", { name: "Книги" });
    move(50, 120);
    expect(document.querySelector(".record-ghost")).toBeInTheDocument();
    expect(document.querySelector(".record-ghost")).toHaveAttribute("aria-hidden", "true");
    expect(document.body).toHaveClass("is-record-dragging");
    expect(screen.getByTestId("over")).toHaveTextContent("books");

    release(50, 120);
    expect(onDrop).toHaveBeenCalledExactlyOnceWith("m1", "books");
    expect(document.querySelector(".record-ghost")).toBeNull();
    expect(document.body).not.toHaveClass("is-record-dragging");
    expect(screen.getByTestId("over")).toHaveTextContent("-");
  });

  it("бросок мимо чатов ничего не делает", () => {
    const onDrop = vi.fn();
    render(<Harness onDrop={onDrop} />);
    press();
    move(400, 300);
    expect(document.querySelector(".record-ghost")).toBeInTheDocument();
    release(400, 300);
    expect(onDrop).not.toHaveBeenCalled();
    expect(document.querySelector(".record-ghost")).toBeNull();
  });

  it("Esc отменяет перетаскивание: бросок после него ничего не делает, окно Esc не получает", () => {
    const onDrop = vi.fn();
    const onWindowKey = vi.fn();
    document.addEventListener("keydown", onWindowKey);
    render(<Harness onDrop={onDrop} />);
    press();
    under = screen.getByRole("button", { name: "Идеи" });
    move(50, 120);
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(document.querySelector(".record-ghost")).toBeNull();
    expect(onWindowKey).not.toHaveBeenCalled();
    release(50, 120);
    expect(onDrop).not.toHaveBeenCalled();
    document.removeEventListener("keydown", onWindowKey);
  });

  it("на чат, куда нельзя (текущий), не подсвечивается и не бросается", () => {
    const onDrop = vi.fn();
    render(<Harness onDrop={onDrop} canDrop={(chatId) => chatId !== "books"} />);
    press();
    under = screen.getByRole("button", { name: "Книги" });
    move(50, 120);
    expect(screen.getByTestId("over")).toHaveTextContent("-");
    release(50, 120);
    expect(onDrop).not.toHaveBeenCalled();
  });

  it("не начинается с кнопки внутри плашки, не основной кнопкой мыши и не касанием", () => {
    const onDrop = vi.fn();
    render(<Harness onDrop={onDrop} />);
    for (const [target, init] of [
      [screen.getByRole("button", { name: "кнопка в плашке" }), {}],
      [plate(), { button: 2 }],
      [plate(), { pointerType: "touch" }],
    ] as const) {
      press(target, init);
      under = screen.getByRole("button", { name: "Книги" });
      move(50, 120);
      expect(document.querySelector(".record-ghost")).toBeNull();
      release(50, 120);
    }
    expect(onDrop).not.toHaveBeenCalled();
  });

  it("после перетаскивания идущий следом щелчок подавляется (меню, ссылка, тег под курсором не срабатывают)", () => {
    render(<Harness onDrop={() => undefined} />);
    const clicked = vi.fn();
    document.addEventListener("click", clicked);
    press();
    move(400, 300);
    release(400, 300);
    fireEvent.click(document.body);
    expect(clicked).not.toHaveBeenCalled();
    document.removeEventListener("click", clicked);
  });
});
