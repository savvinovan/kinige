import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "../../test/render";
import { CARD_TODAY, LONG_TEXT, MANY_YEARS, ONE_YEAR, compactYear, detailedYear } from "./demoCard";
import { OnThisDayCard } from "./OnThisDayCard";

function setup(card = MANY_YEARS, language: "ru" | "en" = "ru") {
  const handlers = { onOpen: vi.fn(), onReply: vi.fn(), onDismiss: vi.fn() };
  render(<OnThisDayCard card={card} {...handlers} />, { language });
  return handlers;
}

describe("Карточка «В этот день»", () => {
  it("заголовок, годы от новых к старым с подписью «N лет назад · дата» и записи со временем", () => {
    setup();
    const region = screen.getByRole("region", { name: "В этот день" });
    const years = within(region).getAllByRole("heading", { level: 4 }).map((h) => h.textContent);
    expect(years).toEqual(["1 год назад · 31 октября 2025", "3 года назад · 31 октября 2023", "4 года назад · 31 октября 2022"]);
    expect(within(region).getByText("Купил билеты на субботу.")).toBeInTheDocument();
    expect(within(region).getAllByText("09:12")).toHaveLength(3);
    expect(within(region).getAllByText("11:12")).toHaveLength(2);
  });

  it("«ещё N записей этого дня» у года, где записей больше двух, и строки старых лет с числом записей", () => {
    setup();
    expect(screen.getByRole("button", { name: /^ещё 1 запись этого дня/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Открыть день 31 октября 2019 в ленте" })).toHaveTextContent("31 октября 2019 · 5 записей");
    expect(screen.getByRole("button", { name: "Открыть день 31 октября 2016 в ленте" })).toHaveTextContent("31 октября 2016 · 1 запись");
  });

  it("множественные формы русского: 1 год, 2 года, 5 лет, 11 лет, 21 год", () => {
    const years = [2025, 2024, 2021, 2015, 2005].map((year) => detailedYear(year, ["запись"]));
    setup({ today: CARD_TODAY, years });
    const headings = screen.getAllByRole("heading", { level: 4 }).map((h) => h.textContent?.split(" · ")[0]);
    expect(headings).toEqual(["1 год назад", "2 года назад", "5 лет назад", "11 лет назад", "21 год назад"]);
  });

  it("множественные формы записей в строках и в «ещё N»: 1, 2, 5, 21", () => {
    const years = [detailedYear(2025, ["а", "б"], 2 + 21), compactYear(2020, 2), compactYear(2019, 5), compactYear(2018, 21)];
    setup({ today: CARD_TODAY, years });
    expect(screen.getByRole("button", { name: /ещё 21 запись этого дня/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /31 октября 2020 в ленте/ })).toHaveTextContent("2 записи");
    expect(screen.getByRole("button", { name: /31 октября 2019 в ленте/ })).toHaveTextContent("5 записей");
    expect(screen.getByRole("button", { name: /31 октября 2018 в ленте/ })).toHaveTextContent("21 запись");
  });

  it("английский: «3 years ago · October 31, 2023», «1 more entry on this day», «5 entries»", () => {
    setup(MANY_YEARS, "en");
    expect(screen.getByRole("region", { name: "On this day" })).toBeInTheDocument();
    expect(screen.getAllByRole("heading", { level: 4 }).map((h) => h.textContent)).toContain("3 years ago · October 31, 2023");
    expect(screen.getByRole("button", { name: /^1 more entry on this day/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open October 31, 2019 in the feed" })).toHaveTextContent("October 31, 2019 · 5 entries");
    expect(screen.getByRole("button", { name: "Hide until tomorrow" })).toBeInTheDocument();
  });

  it("щелчок по записи, «ещё N» и строке старого года открывает запись; крестик скрывает; «Ответить» отдаёт запись", () => {
    const { onOpen, onReply, onDismiss } = setup();
    fireEvent.click(screen.getByRole("button", { name: "Открыть запись 31 октября 2023, 09:12" }));
    expect(onOpen).toHaveBeenLastCalledWith("m-2023-0");
    fireEvent.click(screen.getByRole("button", { name: /^ещё 1 запись этого дня/ }));
    expect(onOpen).toHaveBeenLastCalledWith("m-2025-0");
    fireEvent.click(screen.getByRole("button", { name: "Открыть день 31 октября 2019 в ленте" }));
    expect(onOpen).toHaveBeenLastCalledWith("m-2019-0");
    fireEvent.click(screen.getAllByRole("button", { name: "Ответить на запись 31 октября 2023" })[0] as HTMLElement);
    expect(onReply).toHaveBeenCalledTimes(1);
    expect(onReply.mock.calls[0]?.[0].message.id).toBe("m-2023-0");
    fireEvent.click(screen.getByRole("button", { name: "Скрыть до завтра" }));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it("порядок Tab: запись, «Ответить», …, «ещё N», строки старых лет, крестик стоит первым в заголовке", () => {
    setup();
    const names = screen.getAllByRole("button").map((b) => b.getAttribute("aria-label") ?? b.textContent ?? "");
    expect(names[0]).toBe("Скрыть до завтра");
    const open = names.findIndex((n) => n.startsWith("Открыть запись 31 октября 2025"));
    expect(names[open + 1]).toBe("Ответить на запись 31 октября 2025");
    expect(names.findIndex((n) => n.startsWith("ещё 1 запись"))).toBeGreaterThan(open + 3);
  });

  it("подписи для диктора у записи и у «Ответить» содержат дату", () => {
    setup(ONE_YEAR);
    expect(screen.getByRole("button", { name: "Открыть запись 31 октября 2025, 09:12" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ответить на запись 31 октября 2025" })).toBeInTheDocument();
  });

  it("длинный текст не раздувает строку: он обрезается стилем, а в разметке остаётся целиком", () => {
    setup(LONG_TEXT);
    const text = document.querySelector(".on-this-day__text");
    expect(text?.textContent).toContain("Ещё раз пройтись по конспекту.");
  });

  it("мусорные данные: пустой список лет и год без записей не роняют карточку", () => {
    setup({ today: CARD_TODAY, years: [] });
    expect(screen.getByRole("region", { name: "В этот день" })).toBeInTheDocument();
    expect(screen.queryAllByRole("heading", { level: 4 })).toHaveLength(0);
  });
});
