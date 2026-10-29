import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Окно «Доступ ИИ-агентов» (слайс 4a): одна прокрутка — у панели окна. При системной настройке «всегда показывать полосы
// прокрутки» вложенные полосы (список чатов, блоки кода, журнал) наезжали на текст команды. jsdom не считает вёрстку и не
// подключает CSS, поэтому правила проверяются по самому файлу: у `.agent-*` нет своей прокрутки и предельной высоты, а
// блок кода переносит длинные строки.

const withoutComments = (source: string) => source.replace(/\/\*[\s\S]*?\*\//g, "");
const css = withoutComments(readFileSync(resolve(__dirname, "agents.css"), "utf8"));
const modalCss = withoutComments(readFileSync(resolve(__dirname, "../settings/modal.css"), "utf8"));

/** Правила файла: селектор -> тело. Вложенных блоков в файле нет. */
function rules(source: string): Map<string, string> {
  const result = new Map<string, string>();
  for (const match of source.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const [selector = "", body = ""] = [match[1], match[2]];
    for (const one of selector.split(",")) {
      result.set(one.trim(), `${result.get(one.trim()) ?? ""}${body}`);
    }
  }
  return result;
}

describe("прокрутка в окне «Доступ ИИ-агентов»", () => {
  it("у блоков окна нет своей прокрутки и предельной высоты: список чатов, блоки кода и журнал показываются целиком", () => {
    for (const [selector, body] of rules(css)) {
      expect(body, selector).not.toMatch(/overflow(-x|-y)?\s*:\s*(auto|scroll|overlay)/);
      expect(body, selector).not.toMatch(/(^|[;\s])max-height\s*:/);
    }
    for (const selector of [".agent-chats", ".agent-code__text", ".agent-log__list"]) {
      expect(rules(css).has(selector), selector).toBe(true);
    }
  });

  it("прокручивается только тело самой панели", () => {
    expect(rules(modalCss).get(".modal__body")).toMatch(/overflow-y\s*:\s*auto/);
  });

  // Окно «Доступ ИИ-агентов» открывается поверх «Настроек». Полоса прокрутки нижнего окна в WebKit проступала сквозь верхнее
  // (на снимке 02 второй короткий «язычок» рядом с основной полосой): у окна под другим окном прокрутки быть не должно.
  it("у окна под другим окном нет прокрутки (data-covered)", () => {
    expect(rules(modalCss).get(".modal[data-covered] .modal__body")).toMatch(/overflow-y\s*:\s*hidden/);
  });

  it("блок кода переносит длинную строку (путь к сокету, команда), а не прокручивает", () => {
    const body = rules(css).get(".agent-code__text") ?? "";
    expect(body).toMatch(/white-space\s*:\s*pre-wrap/);
    expect(body).toMatch(/overflow-wrap\s*:\s*anywhere/);
    expect(body).not.toMatch(/white-space\s*:\s*pre\s*;/);
  });
});
