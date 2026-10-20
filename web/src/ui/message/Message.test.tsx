import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Message } from "./Message";

const sentAt = new Date(2026, 9, 20, 9, 12);

describe("Message", () => {
  it("текст и время у каждого сообщения: <time> с машинной датой и 24-часовым видом", () => {
    render(<Message markdown="Купить хлеб" sentAt={sentAt} />);
    expect(screen.getByText("Купить хлеб")).toBeInTheDocument();
    const time = screen.getByText("09:12");
    expect(time.tagName).toBe("TIME");
    expect(time).toHaveAttribute("datetime", sentAt.toISOString());
  });

  it("обычное сообщение: кнопки «Повторить» нет, у конца текста есть распорка под время", () => {
    const { container } = render(<Message markdown="Купить хлеб" sentAt={sentAt} />);
    expect(screen.queryByRole("button")).toBeNull();
    expect(container.querySelector(".message__spacer")).not.toBeNull();
  });

  it("не отправилось: подпись и кнопка «Повторить» — настоящая кнопка, вызывает onRetry", () => {
    const onRetry = vi.fn();
    render(<Message markdown="Купить билеты" sentAt={sentAt} status="failed" onRetry={onRetry} />);
    expect(screen.getByText(/Не отправилось\./)).toBeInTheDocument();
    const retry = screen.getByRole("button", { name: "Повторить отправку сообщения" });
    // Имя кнопки начинается с видимой подписи (WCAG 2.5.3), а сама кнопка берёт фокус и срабатывает как обычная.
    expect(retry).toHaveTextContent("Повторить");
    expect(retry).toHaveAttribute("type", "button");
    retry.focus();
    expect(retry).toHaveFocus();
    fireEvent.click(retry);
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("не отправилось: время остаётся у сообщения, распорки в тексте нет (время стоит рядом с «Повторить»)", () => {
    const { container } = render(<Message markdown="Купить билеты" sentAt={sentAt} status="failed" />);
    expect(screen.getByText("09:12")).toBeInTheDocument();
    expect(container.querySelector(".message__spacer")).toBeNull();
  });

  it("подсвеченное сообщение помечено классом рамки", () => {
    const { container } = render(<Message markdown="Нашлось" sentAt={sentAt} highlighted />);
    expect(container.querySelector("article")).toHaveClass("message--highlighted");
  });

  it("сообщение кончается кодом: время под блоком, а не поверх", () => {
    const { container } = render(<Message markdown={"Вот код:\n\n```\nfn x() {}\n```"} sentAt={sentAt} />);
    expect(container.querySelector("article")).toHaveClass("message--time-below");
  });

  it("опасное содержимое не попадает в DOM", () => {
    const { container } = render(
      <Message markdown={'<img src=x onerror="alert(1)"> [x](javascript:alert(1)) <script>alert(2)</script>'} sentAt={sentAt} />,
    );
    expect(container.querySelector("img, script, a")).toBeNull();
  });

  it("задачи: квадраты с подписью состояния для диктора", () => {
    render(<Message markdown={"- [x] Поход\n- [ ] Фотографии"} sentAt={sentAt} />);
    expect(screen.getByRole("img", { name: "Выполнено" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Не выполнено" })).toBeInTheDocument();
  });
});
