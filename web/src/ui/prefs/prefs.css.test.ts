import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Окно «Настройки» (слайс 4b): одна прокручиваемая область — страница справа (`.prefs__page`). Дерево слева, списки чатов,
// журнал, блоки кода и пути показываются целиком, без своей прокрутки и предельной высоты. Предупреждение при включении —
// `Modal` поверх окна: страница под ним свой overflow не меняет. jsdom не считает вёрстку и не подключает CSS, поэтому правила
// проверяются по самим файлам; настоящую геометрию (полосы видны) проверяет `tests/visual/scrollbars.spec.ts`.

const read = (path: string) => readFileSync(resolve(__dirname, path), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** Файлы стилей, которые подключает окно настроек (страницы, дерево, строки, флажки, журнал, окно предупреждения). */
const FILES = [
  "prefs.css",
  "settings-tree.css",
  "../settings/settings.css",
  "../settings/shortcut-row.css",
  "../settings/backup-row.css",
  "../agents/agents.css",
  "../controls/controls.css",
  "../settings/modal.css",
];

function rules(source: string): Map<string, string> {
  const result = new Map<string, string>();
  for (const match of source.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const [selector = "", body = ""] = [match[1], match[2]];
    for (const one of selector.split(",")) result.set(one.trim(), `${result.get(one.trim()) ?? ""}${body}`);
  }
  return result;
}

const SCROLLS = /overflow(-x|-y)?\s*:\s*(auto|scroll|overlay)/;

describe("прокрутка в окне «Настройки»", () => {
  it("прокручиваются только страница справа и тело окна предупреждения (оно поверх окна)", () => {
    const scrolling: string[] = [];
    for (const file of FILES) {
      for (const [selector, body] of rules(read(file))) if (SCROLLS.test(body)) scrolling.push(selector);
    }
    expect(scrolling.sort()).toEqual([".modal__body", ".prefs__page"]);
  });

  it("дерево, колонка, подэкраны, строки «Настроить ›» и файл настроек прокрутки и предельной высоты не имеют", () => {
    for (const file of ["prefs.css", "settings-tree.css"]) {
      for (const [selector, body] of rules(read(file))) {
        if (selector === ".prefs__page") continue;
        expect(body, `${file} ${selector}`).not.toMatch(SCROLLS);
        expect(body, `${file} ${selector}`).not.toMatch(/(^|[;\s])max-height\s*:/);
      }
    }
  });

  it("страница не сжимается и не растягивает окно: min-width: 0, min-height: 0, колонка ограничена токеном", () => {
    const page = rules(read("prefs.css")).get(".prefs__page") ?? "";
    expect(page).toMatch(/min-width\s*:\s*0/);
    expect(page).toMatch(/min-height\s*:\s*0/);
    expect(page).toMatch(/overflow-y\s*:\s*auto/);
    expect(rules(read("prefs.css")).get(".prefs__column")).toMatch(/max-width\s*:\s*var\(--layout-settings-page-width\)/);
    expect(rules(read("prefs.css")).get(".prefs")).toMatch(/grid-template-columns\s*:\s*var\(--layout-settings-tree-width\)/);
  });

  it("документ окна не прокручивается: строка сетки ровно высота окна, у html окна настроек overflow: hidden", () => {
    const css = rules(read("prefs.css"));
    expect(css.get(".prefs")).toMatch(/grid-template-rows\s*:\s*minmax\(0,\s*1fr\)/);
    expect(css.get('html[data-window="settings"]')).toMatch(/overflow\s*:\s*hidden/);
    expect(css.get('html[data-window="settings"] body')).toMatch(/overflow\s*:\s*hidden/);
    expect(css.get(".prefs")).toMatch(/height\s*:\s*100%/);
  });

  it("кольцо фокуса в окне настроек гасится, пока последним был не ввод с клавиатуры", () => {
    const css = rules(read("prefs.css"));
    expect(css.get('html[data-window="settings"]:not([data-input="keyboard"]) :focus-visible')).toMatch(/outline\s*:\s*none/);
  });

  it("кольцо флажка (рисуется на .checkbox__box, а не на поле) гасится так же и остаётся для клавиатуры", () => {
    const quiet = 'html[data-window="settings"]:not([data-input="keyboard"]) .checkbox__input:focus-visible + .checkbox__box';
    expect(rules(read("prefs.css")).get(quiet)).toMatch(/outline\s*:\s*none/);
    // Само кольцо в дизайн-системе на месте: без клавиатурного ввода его гасит правило выше, с клавиатурой оно видно.
    expect(rules(read("../controls/controls.css")).get(".checkbox__input:focus-visible + .checkbox__box")).toMatch(
      /outline\s*:[^;]*var\(--color-focus-ring\)/,
    );
  });

  it("предупреждение не двигает страницу: подложка и панель закреплены (position: fixed), страница не получает overflow", () => {
    expect(rules(read("../settings/modal.css")).get(".modal")).toMatch(/position\s*:\s*fixed/);
    expect(rules(read("prefs.css")).get(".prefs__page")).not.toMatch(/overflow-y\s*:\s*(hidden|scroll)/);
  });

  it("дерево с наименьшей высотой окна и крупным текстом помещается без прокрутки: пункты в rem, у дерева высота окна", () => {
    const tree = rules(read("settings-tree.css"));
    expect(tree.get(".settings-tree")).toMatch(/height\s*:\s*100%/);
    expect(tree.get(".settings-tree")).not.toMatch(SCROLLS);
    // 7 пунктов по `--layout-control-height` (2,3 rem) с промежутком `--space-1` и полями: 7 × 2,3 + 6 × 0,25 + 2 × 1 rem
    // = 19,6 rem; при крупном тексте (корень 16,25 px) это 319 px < наименьшей высоты окна 440 px.
    const rem = 7 * 2.3 + 6 * 0.25 + 2 * 1;
    expect(rem * 16.25).toBeLessThan(440);
  });
});
