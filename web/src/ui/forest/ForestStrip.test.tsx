import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "../../test/render";
import { FOREST_2026 } from "./fixtures";
import { ForestStrip } from "./ForestStrip";

const open = vi.fn();
const setup = (selected: string | null = "2026-10-16", language: "en" | "ru" = "en") => {
  open.mockClear();
  return render(<ForestStrip table={FOREST_2026} selected={selected} onOpenDay={open} />, { language });
};
const cell = (date: string) => document.querySelector<HTMLElement>(`[data-date="${date}"]`)!;

describe("the collapsed strip", () => {
  it("is a grid with a cell per day up to today; later days are plain ground", () => {
    setup();
    const grid = screen.getByRole("grid", { name: "Days of 2026" });
    const days = FOREST_2026.days.filter((d) => d.date <= FOREST_2026.today);
    expect(within(grid).getAllByRole("gridcell")).toHaveLength(days.length);
    expect(cell("2026-10-22")).toBeNull();
    expect(grid.querySelectorAll(".forest-strip__day--future")).toHaveLength(365 - days.length);
    // Month letters on top.
    expect(within(grid).getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"]);
  });

  it("tints by season and by stage; each cell reads as the announcement of its day", () => {
    setup();
    const day = FOREST_2026.days.find((d) => d.date === "2026-10-16")!;
    expect(cell("2026-10-16")).toHaveAttribute("data-season", "autumn");
    expect(cell("2026-10-16")).toHaveAttribute("data-stage", String(day.stage));
    expect(cell("2026-10-16")).toHaveAccessibleName(/^October 16, \d+ notes, /);
    expect(cell("2026-08-05")).toHaveAccessibleName("August 5, No notes");
  });

  it("outlines the day at the top of the feed", () => {
    setup("2026-03-04");
    expect(cell("2026-03-04")).toHaveAttribute("aria-selected", "true");
    expect(cell("2026-03-04")).toHaveAttribute("data-selected", "true");
    expect(document.querySelectorAll('[data-selected="true"]')).toHaveLength(1);
  });

  it("one cell is in the Tab order: the selected day, or the last selectable one", () => {
    setup("2026-03-04");
    expect(document.querySelectorAll('[role="gridcell"][tabindex="0"]')).toHaveLength(1);
    expect(cell("2026-03-04")).toHaveAttribute("tabindex", "0");
  });

  it("without a selection the last selectable day takes the tab stop", () => {
    setup(null);
    expect(cell("2026-10-21")).toHaveAttribute("tabindex", "0");
  });

  it("up and down move by a day, left and right by a week, and the tab stop follows", () => {
    setup("2026-03-04");
    cell("2026-03-04").focus();
    fireEvent.keyDown(cell("2026-03-04"), { key: "ArrowDown" });
    expect(cell("2026-03-05")).toHaveFocus();
    expect(cell("2026-03-05")).toHaveAttribute("tabindex", "0");
    expect(cell("2026-03-04")).toHaveAttribute("tabindex", "-1");
    fireEvent.keyDown(cell("2026-03-05"), { key: "ArrowRight" });
    expect(cell("2026-03-12")).toHaveFocus();
    fireEvent.keyDown(cell("2026-03-12"), { key: "ArrowUp" });
    fireEvent.keyDown(cell("2026-03-11"), { key: "ArrowLeft" });
    expect(cell("2026-03-04")).toHaveFocus();
  });

  it("does not leave the year or go past today", () => {
    setup("2026-01-01");
    cell("2026-01-01").focus();
    fireEvent.keyDown(cell("2026-01-01"), { key: "ArrowUp" });
    fireEvent.keyDown(cell("2026-01-01"), { key: "ArrowLeft" });
    expect(cell("2026-01-01")).toHaveFocus();
    cell("2026-10-21").focus();
    fireEvent.keyDown(cell("2026-10-21"), { key: "ArrowDown" });
    fireEvent.keyDown(cell("2026-10-21"), { key: "ArrowRight" });
    expect(cell("2026-10-21")).toHaveFocus();
  });

  it("Enter, Space and a click open the day; other keys do nothing", () => {
    setup();
    cell("2026-05-05").focus();
    fireEvent.keyDown(cell("2026-05-05"), { key: "Enter" });
    expect(open).toHaveBeenLastCalledWith("2026-05-05");
    fireEvent.keyDown(cell("2026-05-05"), { key: " " });
    expect(open).toHaveBeenCalledTimes(2);
    fireEvent.keyDown(cell("2026-05-05"), { key: "a" });
    fireEvent.keyDown(cell("2026-05-05"), { key: "Escape" });
    expect(open).toHaveBeenCalledTimes(2);
    fireEvent.click(cell("2026-06-07"));
    expect(open).toHaveBeenLastCalledWith("2026-06-07");
    // The month letter and the future ground are not days.
    fireEvent.click(screen.getAllByRole("columnheader")[0]!);
    fireEvent.click(document.querySelector(".forest-strip__day--future")!);
    expect(open).toHaveBeenCalledTimes(3);
  });

  it("hover shows the card of the day with its first line, and leaving hides it", () => {
    setup();
    expect(document.querySelector(".day-card")).toBeNull();
    fireEvent.pointerOver(cell("2026-10-16"));
    const card = document.querySelector(".day-card")!;
    expect(card).toHaveTextContent("Fri, October 16, 2026");
    expect(card).toHaveTextContent(/The longest synthetic entry/);
    fireEvent.pointerLeave(document.querySelector(".forest-strip__grid")!);
    expect(document.querySelector(".day-card")).toBeNull();
  });

  it("keyboard focus shows the same card", () => {
    setup();
    act(() => cell("2026-02-10").focus());
    expect(document.querySelector(".day-card")).toHaveTextContent("Tue, February 10, 2026");
    act(() => cell("2026-02-10").blur());
    expect(document.querySelector(".day-card")).toBeNull();
  });

  it("is labelled in Russian too", () => {
    setup("2026-10-16", "ru");
    expect(screen.getByRole("grid", { name: "Дни 2026 года" })).toBeInTheDocument();
    expect(cell("2026-08-05")).toHaveAccessibleName("5 августа, Записей нет");
  });
});
