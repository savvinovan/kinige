import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// The "Доступ ИИ-агентов" window (slice 4a): one scroll, the window panel's. With the system setting "always show scroll
// bars" nested scrollbars (the chat list, code blocks, the log) overlapped the command text. jsdom does not compute layout and does not
// load CSS, so the rules are checked against the file itself: `.agent-*` has no scroll of its own or a maximum height, and the
// code block wraps long lines.

const withoutComments = (source: string) => source.replace(/\/\*[\s\S]*?\*\//g, "");
const css = withoutComments(readFileSync(resolve(__dirname, "agents.css"), "utf8"));
const modalCss = withoutComments(readFileSync(resolve(__dirname, "../settings/modal.css"), "utf8"));

/** The file's rules: selector -> body. There are no nested blocks in the file. */
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

  // The "Доступ ИИ-агентов" window opens over "Настройки". The lower window's scrollbar in WebKit showed through the upper one
  // (in screenshot 02 a second short "tab" next to the main scrollbar): a window under another window must not scroll.
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
