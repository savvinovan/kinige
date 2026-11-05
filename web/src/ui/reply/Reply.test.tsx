import { describe, expect, it, vi } from "vitest";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { fireEvent, render, screen } from "../../test/render";
import { ReplyQuote } from "./ReplyQuote";
import { ReplyStrip } from "./ReplyStrip";

const LIVE: ReplyTo = {
  id: "old",
  chatId: "chat",
  journalDate: "2023-10-31",
  excerpt: "Так будет лучше вторая строка",
  state: "live",
  yearsLater: 3,
  attachments: { photos: 0, files: 0, voices: 0, voiceMs: null, name: null },
};

describe("Цитата ответа", () => {
  it("живая: «3 года спустя · 31 октября 2023» и начало старой записи; щелчок открывает запись", () => {
    const onOpen = vi.fn();
    render(<ReplyQuote quote={LIVE} onOpen={onOpen} />);
    const quote = screen.getByRole("button", { name: "3 года спустя, ответ на запись 31 октября 2023" });
    expect(quote).toHaveTextContent("3 года спустя · 31 октября 2023");
    expect(quote).toHaveTextContent("Так будет лучше вторая строка");
    expect(quote).toHaveAccessibleDescription("Так будет лучше вторая строка");
    fireEvent.click(quote);
    expect(onOpen).toHaveBeenCalledWith(LIVE);
  });

  it("меньше года: «Ответ на запись · дата» без подписи «N лет спустя»", () => {
    render(<ReplyQuote quote={{ ...LIVE, journalDate: "2023-03-14", yearsLater: 0 }} onOpen={() => undefined} />);
    const quote = screen.getByRole("button", { name: "Ответ на запись 14 марта 2023" });
    expect(quote).toHaveTextContent("Ответ на запись · 14 марта 2023");
    expect(quote.textContent).not.toMatch(/спустя/);
  });

  it("формы русского: 1 год, 2 года, 5 лет, 21 год", () => {
    const labels = [1, 2, 5, 21].map((yearsLater) => {
      const { container, unmount } = render(<ReplyQuote quote={{ ...LIVE, yearsLater }} />);
      const text = container.querySelector(".reply-quote__label")?.textContent;
      unmount();
      return text?.split(" · ")[0];
    });
    expect(labels).toEqual(["1 год спустя", "2 года спустя", "5 лет спустя", "21 год спустя"]);
  });

  it("в корзине: «Запись в корзине», не нажимается; удалена навсегда: «Исходная запись удалена»", () => {
    const { unmount } = render(<ReplyQuote quote={{ ...LIVE, state: "trash" }} onOpen={() => undefined} />);
    expect(screen.getByText("Запись в корзине")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByText(/Так будет/)).toBeNull();
    unmount();
    render(<ReplyQuote quote={{ ...LIVE, state: "missing", chatId: null, journalDate: null, excerpt: "", yearsLater: 0 }} onOpen={() => undefined} />);
    expect(screen.getByText("Исходная запись удалена")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("без обработчика цитата — обычный блок", () => {
    render(<ReplyQuote quote={LIVE} />);
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("3 года спустя · 31 октября 2023")).toBeInTheDocument();
  });

  it("английский: «3 years later · October 31, 2023» и подпись для диктора", () => {
    render(<ReplyQuote quote={LIVE} onOpen={() => undefined} />, { language: "en" });
    expect(screen.getByRole("button", { name: "3 years later, reply to the entry of October 31, 2023" })).toHaveTextContent(
      "3 years later · October 31, 2023",
    );
  });

  it("имя автора чужой реплики (импорт) остаётся в начале цитаты", () => {
    render(<ReplyQuote quote={{ ...LIVE, yearsLater: 0, excerpt: "Анна К. Ну что, едем в субботу?" }} onOpen={() => undefined} />);
    expect(screen.getByRole("button")).toHaveTextContent("Анна К. Ну что, едем в субботу?");
  });
});

describe("Строка «Ответ на запись» над полем", () => {
  it("дата, первая строка записи и крестик «Отменить ответ»", () => {
    const onCancel = vi.fn();
    render(<ReplyStrip quote={LIVE} onCancel={onCancel} />);
    const strip = screen.getByRole("group", { name: "Ответ на запись 31 октября 2023" });
    expect(strip).toHaveTextContent("Ответ на запись · 31 октября 2023");
    expect(strip).toHaveTextContent("Так будет лучше вторая строка");
    fireEvent.click(screen.getByRole("button", { name: "Отменить ответ" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("запись в корзине и удалённая: строка говорит об этом, текста записи нет", () => {
    const { unmount } = render(<ReplyStrip quote={{ ...LIVE, state: "trash" }} onCancel={() => undefined} />);
    expect(screen.getByText("Запись в корзине")).toBeInTheDocument();
    expect(screen.queryByText(/Так будет/)).toBeNull();
    unmount();
    render(<ReplyStrip quote={{ ...LIVE, state: "missing", journalDate: null, excerpt: "" }} onCancel={() => undefined} />);
    expect(screen.getByText("Исходная запись удалена")).toBeInTheDocument();
  });

  it("английский", () => {
    render(<ReplyStrip quote={LIVE} onCancel={() => undefined} />, { language: "en" });
    expect(screen.getByText("Replying to the entry of October 31, 2023")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel the reply" })).toBeInTheDocument();
  });
});
