import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DEMO_CHATS, demoMessages } from "../../api/demoData";
import { Feed } from "./Feed";

const NOW = new Date(2026, 9, 20, 17, 0);

describe("Feed", () => {
  it("одна метка на день: «14 марта 2025», «Вчера», «Сегодня»; без дня недели и без второй строки с датой", () => {
    render(<Feed messages={demoMessages(NOW)} now={NOW} />);
    const labels = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(labels).toEqual(["14 марта 2025", "Вчера", "Сегодня"]);
    expect(screen.queryByText(/вторник|понедельник|пятница|20 октября/i)).toBeNull();
  });

  it("над лентой нет заголовка и названия чата (выбранный чат виден в сайдбаре)", () => {
    render(<Feed messages={demoMessages(NOW)} now={NOW} />);
    expect(screen.queryByRole("heading", { level: 1 })).toBeNull();
    for (const chat of DEMO_CHATS) expect(screen.queryByText(chat.name)).toBeNull();
  });

  it("у каждого сообщения своё время, слияния в «страницу» нет (09:12 и 09:14 — два сообщения)", () => {
    render(<Feed messages={demoMessages(NOW)} now={NOW} />);
    const articles = screen.getAllByRole("article");
    expect(articles).toHaveLength(8);
    for (const article of articles) expect(article.querySelector("time")).not.toBeNull();
    expect(screen.getAllByText("09:12")).toHaveLength(2);
    expect(screen.getByText("09:14")).toBeInTheDocument();
  });

  it("серия: плашка после короткой паузы получает класс серии, первая и после длинной паузы — нет", () => {
    const { container } = render(<Feed messages={demoMessages(NOW)} now={NOW} />);
    const items = [...container.querySelectorAll("li.feed__item:not(.feed__item--day)")];
    const series = (text: string) =>
      items.find((li) => li.textContent?.includes(text))?.classList.contains("feed__item--series");
    // 09:12 -> 09:14 — серия; 09:14 -> 11:30 — уже нет.
    expect(series("Ещё: позвонить")).toBe(true);
    expect(series("Планы на выходные")).toBe(false);
    expect(series("Проснулся рано")).toBe(false);
  });

  it("пусто: подсказка, ни меток, ни списка", () => {
    render(<Feed messages={[]} now={NOW} />);
    expect(screen.getByText(/Пока пусто/)).toBeInTheDocument();
    expect(screen.queryByRole("list")).toBeNull();
  });

  it("прокручиваемая область берёт фокус с клавиатуры и подписана", () => {
    render(<Feed messages={demoMessages(NOW)} now={NOW} />);
    const region = screen.getByRole("region", { name: "Записи" });
    expect(region).toHaveAttribute("tabindex", "0");
  });

  it("«Повторить» у не отправленного сообщения передаёт его id", () => {
    const onRetry = vi.fn();
    render(<Feed messages={demoMessages(NOW)} now={NOW} onRetry={onRetry} />);
    const failed = screen.getByText("Купить билеты на субботу").closest("article") as HTMLElement;
    fireEvent.click(within(failed).getByRole("button", { name: /Повторить/ }));
    expect(onRetry).toHaveBeenCalledWith("m8");
  });
});
