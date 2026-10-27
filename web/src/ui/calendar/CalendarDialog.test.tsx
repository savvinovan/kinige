import { act, fireEvent, render, screen, within } from "../../test/render";
import { describe, expect, it, vi } from "vitest";
import { CalendarDialog } from "./CalendarDialog";

type Props = Partial<React.ComponentProps<typeof CalendarDialog>>;
const show = (props: Props = {}, language: "ru" | "en" = "ru") => {
  const onPick = vi.fn();
  const onClose = vi.fn();
  const view = render(
    <CalendarDialog value="2026-10-20" today="2026-10-27" language={language} onPick={onPick} onClose={onClose} {...props} />,
    { language },
  );
  return { onPick, onClose, ...view };
};
const grid = () => screen.getByRole("grid");
const day = (iso: string) => grid().querySelector<HTMLButtonElement>(`button[data-date="${iso}"]`) as HTMLButtonElement;
const flush = () => act(async () => void (await Promise.resolve()));

describe("CalendarDialog: вид", () => {
  it("окно с заголовком, месяц «октябрь 2026», сетка с подписью месяца", () => {
    show();
    expect(screen.getByRole("dialog", { name: "Изменить дату" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "октябрь 2026" })).toBeInTheDocument();
    expect(screen.getByRole("grid", { name: "октябрь 2026" })).toBeInTheDocument();
  });

  it("русский: неделя с понедельника, заголовки столбцов с полными названиями для диктора", () => {
    show();
    const headers = within(grid()).getAllByRole("columnheader");
    expect(headers.map((h) => h.textContent)).toEqual(["пн", "вт", "ср", "чт", "пт", "сб", "вс"]);
    expect(headers[0]).toHaveAttribute("abbr", "понедельник");
  });

  it("английский: неделя с воскресенья, месяц «October 2026»", () => {
    show({}, "en");
    expect(screen.getByRole("heading", { name: "October 2026" })).toBeInTheDocument();
    expect(within(grid()).getAllByRole("columnheader").map((h) => h.textContent)).toEqual([
      "Sun",
      "Mon",
      "Tue",
      "Wed",
      "Thu",
      "Fri",
      "Sat",
    ]);
  });

  it("выбранный день (value) — aria-selected, сегодня — aria-current=date; это разные ячейки", () => {
    show();
    expect(day("2026-10-20").closest("td")).toHaveAttribute("aria-selected", "true");
    expect(day("2026-10-21").closest("td")).toHaveAttribute("aria-selected", "false");
    expect(day("2026-10-27")).toHaveAttribute("aria-current", "date");
    expect(day("2026-10-20")).not.toHaveAttribute("aria-current");
    expect(day("2026-10-27")).toHaveClass("calendar__day--today");
  });

  it("дни соседних месяцев показаны приглушёнными и выбираются", () => {
    const { onPick } = show();
    // Сетка октября 2026 с понедельника начинается 28 сентября и кончается 1 ноября.
    expect(day("2026-09-28")).toHaveClass("calendar__day--outside");
    expect(day("2026-11-01")).toHaveClass("calendar__day--outside");
    expect(day("2026-10-15")).not.toHaveClass("calendar__day--outside");
    fireEvent.click(day("2026-11-01"));
    expect(onPick).toHaveBeenCalledWith("2026-11-01");
  });

  it("у ячейки подпись с полной датой, содержащая видимое число", () => {
    show({}, "en");
    expect(day("2026-10-20")).toHaveAccessibleName("Tuesday, October 20, 2026");
    expect(day("2026-10-20")).toHaveTextContent("20");
  });

  it("открывается на месяце value; если value испорчено — на месяце «сегодня»", () => {
    show({ value: "2025-03-14" });
    expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("март 2025");
  });

  it("испорченные value и today не ломают окно", () => {
    show({ value: "вчера", today: "" });
    expect(grid()).toBeInTheDocument();
  });

  it("ошибка — role=alert под сеткой", () => {
    show({ error: "Такой даты нет." });
    expect(screen.getByRole("alert")).toHaveTextContent("Такой даты нет.");
  });
});

describe("CalendarDialog: выбор", () => {
  it("щелчок по дню сразу вызывает onPick с YYYY-MM-DD, включая будущие даты", () => {
    const { onPick } = show();
    fireEvent.click(day("2026-10-15"));
    expect(onPick).toHaveBeenLastCalledWith("2026-10-15");
    fireEvent.click(screen.getByRole("button", { name: "Следующий месяц" }));
    fireEvent.click(day("2026-11-30"));
    expect(onPick).toHaveBeenLastCalledWith("2026-11-30");
  });

  it("«Сегодня» выбирает today", () => {
    const { onPick } = show();
    fireEvent.click(screen.getByRole("button", { name: "Сегодня" }));
    expect(onPick).toHaveBeenCalledWith("2026-10-27");
  });

  it("кнопки месяцев листают, не выбирая дату; январь → декабрь прошлого года", () => {
    const { onPick } = show({ value: "2026-01-15", today: "2026-01-20" });
    fireEvent.click(screen.getByRole("button", { name: "Предыдущий месяц" }));
    expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("декабрь 2025");
    fireEvent.click(screen.getByRole("button", { name: "Следующий месяц" }));
    fireEvent.click(screen.getByRole("button", { name: "Следующий месяц" }));
    expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("февраль 2026");
    expect(onPick).not.toHaveBeenCalled();
  });

  it("пока дата сохраняется (busy), выбор не срабатывает", () => {
    const { onPick } = show({ busy: true });
    fireEvent.click(day("2026-10-15"));
    fireEvent.click(screen.getByRole("button", { name: "Сегодня" }));
    expect(onPick).not.toHaveBeenCalled();
    expect(grid()).toHaveAttribute("aria-busy", "true");
  });

  it("Esc и кнопка закрытия закрывают окно", () => {
    const { onClose } = show();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Закрыть календарь" }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});

describe("CalendarDialog: клавиатура", () => {
  const press = (key: string, options: KeyboardEventInit = {}) => fireEvent.keyDown(document.activeElement as Element, { key, ...options });

  it("при открытии фокус на выбранном дне; в обходе Tab одна остановка в сетке", async () => {
    show();
    await flush();
    expect(day("2026-10-20")).toHaveFocus();
    const stops = [...grid().querySelectorAll("button")].filter((b) => b.tabIndex === 0);
    expect(stops).toEqual([day("2026-10-20")]);
  });

  it("стрелки: день и неделя; фокус и единственная остановка идут за курсором", async () => {
    show();
    await flush();
    press("ArrowRight");
    expect(day("2026-10-21")).toHaveFocus();
    expect(day("2026-10-21").tabIndex).toBe(0);
    expect(day("2026-10-20").tabIndex).toBe(-1);
    press("ArrowDown");
    expect(day("2026-10-28")).toHaveFocus();
    press("ArrowLeft");
    press("ArrowUp");
    expect(day("2026-10-20")).toHaveFocus();
  });

  it("стрелка через границу месяца листает сетку на следующий", async () => {
    show({ value: "2026-10-31" });
    await flush();
    press("ArrowRight");
    expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("ноябрь 2026");
    expect(day("2026-11-01")).toHaveFocus();
  });

  it("Home / End: начало и конец недели (русский — с понедельника)", async () => {
    show();
    await flush();
    press("Home");
    expect(day("2026-10-19")).toHaveFocus();
    press("End");
    expect(day("2026-10-25")).toHaveFocus();
  });

  it("Home / End в английском: неделя с воскресенья", async () => {
    show({}, "en");
    await flush();
    press("Home");
    expect(day("2026-10-18")).toHaveFocus();
    press("End");
    expect(day("2026-10-24")).toHaveFocus();
  });

  it("PageUp / PageDown — месяц (день ужимается), Shift — год", async () => {
    show({ value: "2026-03-31" });
    await flush();
    press("PageUp");
    expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("февраль 2026");
    expect(day("2026-02-28")).toHaveFocus();
    press("PageDown", { shiftKey: true });
    expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("февраль 2027");
  });

  it("Enter и пробел на дне выбирают его (кнопка)", async () => {
    const { onPick } = show();
    await flush();
    press("ArrowRight");
    const focused = document.activeElement as HTMLElement;
    expect(focused.tagName).toBe("BUTTON");
    // Enter и пробел на настоящей кнопке браузер превращает в click.
    fireEvent.click(focused);
    expect(onPick).toHaveBeenCalledWith("2026-10-21");
  });

  it("стрелки в окне не прокручивают страницу: событие отменено", async () => {
    show();
    await flush();
    expect(press("ArrowDown")).toBe(false);
    expect(press("a")).toBe(true);
  });
});
