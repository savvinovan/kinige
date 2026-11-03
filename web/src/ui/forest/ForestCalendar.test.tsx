import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "../../test/render";
import { FOREST_2026, FOREST_2028_SUNDAY } from "./fixtures";
import { ForestCalendar } from "./ForestCalendar";

const open = vi.fn();
const setup = (selected: string | null = "2026-10-16", table = FOREST_2026, language: "en" | "ru" = "en") => {
  open.mockClear();
  return render(<ForestCalendar table={table} selected={selected} onOpenDay={open} />, { language });
};
const day = (date: string) => document.querySelector<HTMLElement>(`[data-date="${date}"]`)!;

describe("the calendar alternative", () => {
  it("is twelve month grids with a button per day up to today", () => {
    setup();
    const group = screen.getByRole("group", { name: "Calendar of 2026" });
    expect(within(group).getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual([
      "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December",
    ]);
    const days = FOREST_2026.days.filter((d) => d.date <= FOREST_2026.today);
    expect(within(group).getAllByRole("button")).toHaveLength(days.length);
    expect(group.querySelectorAll(".forest-calendar__day--future")).toHaveLength(365 - days.length);
  });

  it("each day reads as 'October 16, N notes, a stage' and carries the stage for the dot", () => {
    setup();
    const real = FOREST_2026.days.find((d) => d.date === "2026-10-16")!;
    expect(day("2026-10-16")).toHaveAccessibleName(new RegExp(`^October 16, ${real.notes} notes, `));
    expect(day("2026-10-16")).toHaveAttribute("data-stage", String(real.stage));
    expect(day("2026-10-16")).toHaveAttribute("data-season", "autumn");
    expect(day("2026-08-05")).toHaveAccessibleName("August 5, No notes");
    expect(day("2026-10-16")).toHaveAttribute("aria-current", "date");
  });

  it("places the first of each month under its weekday (Monday first: 1 October 2026 is a Thursday)", () => {
    setup();
    expect(day("2026-10-01").style.gridColumnStart).toBe("4");
    expect(day("2026-06-01").style.gridColumnStart).toBe("1"); // a Monday
    expect(day("2026-10-02").style.gridColumnStart).toBe("");
  });

  it("starts Sunday-first weeks in the first column", () => {
    setup("2028-06-15", FOREST_2028_SUNDAY);
    // 1 January 2028 is a Saturday: the last column.
    expect(day("2028-01-01").style.gridColumnStart).toBe("7");
    expect(day("2028-05-01").style.gridColumnStart).toBe("2"); // a Monday: the second column when the week starts on Sunday
  });

  it("arrows are the board's: left and right a day, up and down a week", () => {
    setup("2026-03-10");
    day("2026-03-10").focus();
    fireEvent.keyDown(day("2026-03-10"), { key: "ArrowRight" });
    expect(day("2026-03-11")).toHaveFocus();
    fireEvent.keyDown(day("2026-03-11"), { key: "ArrowDown" });
    expect(day("2026-03-18")).toHaveFocus();
    fireEvent.keyDown(day("2026-03-18"), { key: "ArrowUp" });
    fireEvent.keyDown(day("2026-03-11"), { key: "ArrowLeft" });
    expect(day("2026-03-10")).toHaveFocus();
    // Across a month boundary.
    day("2026-03-31").focus();
    fireEvent.keyDown(day("2026-03-31"), { key: "ArrowRight" });
    expect(day("2026-04-01")).toHaveFocus();
  });

  it("a click and Enter do what a click on a tree does", () => {
    setup();
    fireEvent.click(day("2026-04-12"));
    expect(open).toHaveBeenLastCalledWith("2026-04-12");
    day("2026-04-13").focus();
    fireEvent.keyDown(day("2026-04-13"), { key: "Enter" });
    expect(open).toHaveBeenLastCalledWith("2026-04-13");
  });

  it("days after today cannot be opened", () => {
    setup();
    expect(day("2026-10-22")).toBeNull();
    fireEvent.click(document.querySelector(".forest-calendar__day--future")!);
    expect(open).not.toHaveBeenCalled();
  });

  it("is in Russian with Russian month names", () => {
    setup("2026-10-16", FOREST_2026, "ru");
    expect(screen.getByRole("group", { name: "Календарь 2026 года" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 3, name: "октябрь" })).toBeInTheDocument();
  });
});
