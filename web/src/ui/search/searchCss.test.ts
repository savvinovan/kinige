import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Поведение, которое видно только в настоящем окне (jsdom CSS не считает): правила не должны пропасть из таблиц.
const css = (name: string) => readFileSync(new URL(name, import.meta.url), "utf8");

describe("Поиск: правила CSS", () => {
  it("область результатов резервирует место под полосу прокрутки: строка фильтров не прыгает", () => {
    const search = css("./search.css");
    const rule = /\.search \{[^}]*\}/.exec(search)?.[0] ?? "";
    expect(rule).toContain("scrollbar-gutter: stable");
  });

  it("у поля поиска сайдбара нет родной кнопки очистки WebKit", () => {
    const sidebar = css("../sidebar/sidebar.css");
    expect(sidebar).toMatch(/\.sidebar__search input::-webkit-search-cancel-button \{\s*appearance: none;/);
  });
});
