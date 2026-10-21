import { fireEvent, render, screen } from "../../test/render";
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
    expect(screen.queryByRole("button", { name: /Повторить/ })).toBeNull();
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

  it("подсвеченное сообщение помечено классом заливки", () => {
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

  describe("ссылки не уводят webview (D1)", () => {
    const withLink = "Прочитал [The Psychology of Habits — fs.blog](https://fs.blog/habits) и записал вывод";

    it("ссылка остаётся ссылкой: адрес, rel, фокус с клавиатуры", () => {
      render(<Message markdown={withLink} sentAt={sentAt} />);
      const link = screen.getByRole("link", { name: "The Psychology of Habits — fs.blog" });
      expect(link).toHaveAttribute("href", "https://fs.blog/habits");
      expect(link).toHaveAttribute("rel", "noopener noreferrer");
      link.focus();
      expect(link).toHaveFocus();
    });

    it("щелчок отменяет переход, адрес страницы не меняется", () => {
      const before = window.location.href;
      render(<Message markdown={withLink} sentAt={sentAt} />);
      const link = screen.getByRole("link");
      // fireEvent возвращает false, если обработчик вызвал preventDefault.
      expect(fireEvent.click(link)).toBe(false);
      expect(window.location.href).toBe(before);
    });

    it("щелчок по тексту внутри ссылки (вложенный <strong>) и средняя кнопка тоже отменяются", () => {
      render(<Message markdown="[**жирная** ссылка](https://example.com/x)" sentAt={sentAt} />);
      const inner = screen.getByText("жирная");
      expect(fireEvent.click(inner)).toBe(false);
      expect(fireEvent(inner, new MouseEvent("auxclick", { bubbles: true, cancelable: true, button: 1 }))).toBe(false);
    });

    it("щелчок вне ссылки не отменяется (выделение текста, фокус)", () => {
      render(<Message markdown={withLink} sentAt={sentAt} />);
      expect(fireEvent.click(screen.getByText(/и записал вывод/))).toBe(true);
    });

    it("щелчок отдаёт адрес в onOpenLink (системный браузер), средняя кнопка — нет", () => {
      const onOpenLink = vi.fn();
      render(<Message markdown={withLink} sentAt={sentAt} onOpenLink={onOpenLink} />);
      const link = screen.getByRole("link");
      fireEvent.click(link);
      expect(onOpenLink).toHaveBeenCalledExactlyOnceWith("https://fs.blog/habits");
      fireEvent(link, new MouseEvent("auxclick", { bubbles: true, cancelable: true, button: 1 }));
      expect(onOpenLink).toHaveBeenCalledTimes(1);
    });

    it("Enter на ссылке (браузер превращает его в click) тоже открывает адрес, окно не уходит", () => {
      const onOpenLink = vi.fn();
      render(<Message markdown={"[текст](mailto:me@example.com)"} sentAt={sentAt} onOpenLink={onOpenLink} />);
      const link = screen.getByRole("link");
      link.focus();
      expect(fireEvent.click(link)).toBe(false);
      expect(onOpenLink).toHaveBeenCalledWith("mailto:me@example.com");
    });
  });

  it("время показывается в поясе записи, а не в поясе окна; на английском тоже 24 часа", () => {
    const moment = new Date(Date.UTC(2026, 9, 21, 6, 12));
    const { unmount } = render(<Message markdown="x" sentAt={moment} timeZone="Europe/Moscow" />);
    expect(screen.getByText("09:12")).toHaveAttribute("datetime", moment.toISOString());
    unmount();
    render(<Message markdown="x" sentAt={moment} timeZone="America/Los_Angeles" />, { language: "en" });
    expect(screen.getByText("23:12")).toBeInTheDocument();
  });

  it("задачи: квадраты с подписью состояния для диктора", () => {
    render(<Message markdown={"- [x] Поход\n- [ ] Фотографии"} sentAt={sentAt} />);
    expect(screen.getByRole("img", { name: "Выполнено" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Не выполнено" })).toBeInTheDocument();
  });
});
