import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

// Цвета, размеры, шрифты, время и ширины живут только в design/tokens -> tokens.css / themes.css (§11.1 ТЗ).
//
// Что считается нарушением (в CSS и в TS/TSX, вне комментариев):
//  - цвета: hex, функции цвета, основные именованные цвета CSS в цветовых свойствах и в JSX-атрибутах;
//  - единицы: px, rem, em, pt, ch, ms, s, vh, vw, vmin, vmax;
//  - font-family и font-size не из var(); line-height числом; opacity не из var() (кроме 0 и 1); stroke-width не из var();
//  - числа в инлайн-стилях TSX: `style={{ fontSize: 18 }}`, `width: 0.5` и подобное.
// Разумные исключения (не нарушения): голый `0`; `opacity: 0` и `opacity: 1`; проценты и `fr` (доля родителя или
// дорожки сетки, не размер); `transparent` и `currentColor`; безразмерные множители внутри `calc()` вокруг токенов
// (`calc(var(--space-8) * 3)`).
const SRC = join(import.meta.dirname, "..");
const SKIP = new Set(["ui/tokens.css", "ui/themes.css", "api/generated"]);

type Kind = "css" | "script";

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    const rel = relative(SRC, path);
    if (SKIP.has(rel)) return [];
    if (statSync(path).isDirectory()) return files(path);
    return /\.(css|tsx?)$/.test(name) && !name.endsWith("no-raw-values.test.ts") ? [path] : [];
  });
}

const COLOR_NAMES =
  "black|white|red|green|blue|yellow|orange|purple|pink|gray|grey|silver|maroon|olive|lime|aqua|cyan|teal|navy|" +
  "fuchsia|magenta|brown|gold|coral|crimson|indigo|violet|salmon|tomato|khaki|ivory|beige|tan|plum|orchid|" +
  "turquoise|lavender|azure|wheat|linen|snow";
const COLOR_PROPS =
  "color|background|background-color|border(?:-[a-z]+)*|outline(?:-color)?|fill|stroke|box-shadow|text-shadow|caret-color|accent-color";
const COLOR_KEYS = "color|background|backgroundColor|border|borderColor|outline|outlineColor|fill|stroke|boxShadow|caretColor";
const SIZE_KEYS =
  "fontSize|lineHeight|width|height|minWidth|maxWidth|minHeight|maxHeight|top|left|right|bottom|inset|gap|rowGap|columnGap|" +
  "margin\\w*|padding\\w*|borderRadius|borderWidth|letterSpacing|flexBasis|strokeWidth";

interface Rule {
  name: string;
  pattern: RegExp;
  only?: Kind;
  /** Проверять текст без выражений var(...): имена внутри имён переменных не считаются. */
  withoutVars?: boolean;
}

const RULES: Rule[] = [
  { name: "hex-цвет", pattern: /#[0-9a-fA-F]{3,8}\b/ },
  { name: "функция цвета", pattern: /\b(rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch|color-mix)\(/ },
  {
    name: "именованный цвет в CSS",
    pattern: new RegExp(`(?:^|[;{\\s])(?:${COLOR_PROPS})\\s*:[^;{}]*\\b(?:${COLOR_NAMES})\\b`, "im"),
    only: "css",
    withoutVars: true,
  },
  {
    name: "именованный цвет в TSX",
    pattern: new RegExp(
      `\\b(?:${COLOR_KEYS})\\s*:\\s*["'\`][^"'\`]*\\b(?:${COLOR_NAMES})\\b|\\b(?:fill|stroke|color)=["'](?:${COLOR_NAMES})["']`,
      "i",
    ),
    only: "script",
    withoutVars: true,
  },
  { name: "размер или время в px/rem/em/pt/ch/ms/s/vh/vw", pattern: /\b\d*\.?\d+(px|rem|em|pt|ch|ms|s|vh|vw|vmin|vmax)\b/ },
  { name: "font-family вне var()", pattern: /font-family\s*:\s*(?!var\()\S/ },
  { name: "font-size вне var()", pattern: /font-size\s*:\s*(?!var\()\S/ },
  { name: "line-height числом", pattern: /\b(?:line-height|lineHeight)\s*:\s*-?\.?\d/ },
  { name: "opacity вне var() (кроме 0 и 1)", pattern: /\bopacity\s*:\s*(?!var\(|(?:0|1)\s*(?:;|,|\}|$))\S/m },
  { name: "stroke-width вне var()", pattern: /stroke-width\s*:\s*(?!var\()\S/ },
  {
    name: "число в инлайн-стиле TSX",
    pattern: new RegExp(`\\b(?:${SIZE_KEYS})\\s*:\\s*-?(?:0?\\.\\d+|[1-9]\\d*\\.?\\d*)`),
    only: "script",
  },
];

function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");
}

/** Имена нарушенных правил для текста файла. */
function violations(source: string, kind: Kind): string[] {
  const text = stripComments(source);
  const withoutVars = text.replace(/var\([^)]*\)/g, "var()");
  return RULES.filter((rule) => !rule.only || rule.only === kind).flatMap((rule) =>
    rule.pattern.test(rule.withoutVars ? withoutVars : text) ? [rule.name] : [],
  );
}

describe("сырые значения вне tokens.css", () => {
  for (const file of files(SRC)) {
    it(relative(SRC, file), () => {
      const kind: Kind = file.endsWith(".css") ? "css" : "script";
      expect(violations(readFileSync(file, "utf8"), kind)).toEqual([]);
    });
  }
});

// Правила сами должны краснеть на каждом классе нарушений и не краснеть на исключениях.
describe("правила ловят нарушения", () => {
  const BAD: [rule: string, kind: Kind, snippet: string][] = [
    ["hex-цвет", "css", "a { color: #fff; }"],
    ["hex-цвет", "script", 'const c = "#1a1a1a";'],
    ["функция цвета", "css", "a { color: rgb(0 0 0); }"],
    ["именованный цвет в CSS", "css", "a { color: red; }"],
    ["именованный цвет в CSS", "css", "a { border: var(--border-width-hairline) solid Gray; }"],
    ["именованный цвет в TSX", "script", '<p style={{ color: "white" }} />'],
    ["именованный цвет в TSX", "script", '<path fill="black" />'],
    ["размер или время в px/rem/em/pt/ch/ms/s/vh/vw", "css", "a { width: 12px; }"],
    ["размер или время в px/rem/em/pt/ch/ms/s/vh/vw", "css", "a { max-width: 60ch; }"],
    ["размер или время в px/rem/em/pt/ch/ms/s/vh/vw", "css", "a { transition: opacity 200ms; }"],
    ["размер или время в px/rem/em/pt/ch/ms/s/vh/vw", "css", "a { animation-duration: 0.3s; }"],
    ["размер или время в px/rem/em/pt/ch/ms/s/vh/vw", "css", "a { height: 50vh; }"],
    ["размер или время в px/rem/em/pt/ch/ms/s/vh/vw", "script", 'const w = "40vw";'],
    ["font-family вне var()", "css", "a { font-family: serif; }"],
    ["font-size вне var()", "css", "a { font-size: larger; }"],
    ["line-height числом", "css", "a { line-height: 1.5; }"],
    ["line-height числом", "script", "<p style={{ lineHeight: 1.4 }} />"],
    ["opacity вне var() (кроме 0 и 1)", "css", "a { opacity: 0.5; }"],
    ["opacity вне var() (кроме 0 и 1)", "script", "<p style={{ opacity: 0.6 }} />"],
    ["stroke-width вне var()", "css", "path { stroke-width: 3; }"],
    ["число в инлайн-стиле TSX", "script", "<p style={{ fontSize: 18 }} />"],
    ["число в инлайн-стиле TSX", "script", "<p style={{ width: 0.5 }} />"],
    ["число в инлайн-стиле TSX", "script", "<p style={{ marginTop: 12 }} />"],
  ];

  for (const [rule, kind, snippet] of BAD) {
    it(`${rule}: ${snippet}`, () => {
      expect(violations(snippet, kind)).toContain(rule);
    });
  }

  const OK: [kind: Kind, snippet: string][] = [
    ["css", "a { margin: 0; padding: 0; }"],
    ["css", "a { width: 100%; grid-template-columns: 1fr auto; }"],
    ["css", "a { color: var(--color-text-primary); background: transparent; border-color: currentColor; }"],
    ["css", "a { opacity: 0; } b { opacity: 1; } c { opacity: var(--opacity-muted); }"],
    ["css", "a { line-height: var(--type-body-line-height); font-size: var(--type-body-size); }"],
    ["css", "a { grid-template-columns: calc(var(--space-8) * 3) 1fr; transition-duration: var(--motion-duration-short); }"],
    ["css", "a { max-width: var(--layout-measure-text); stroke-width: var(--border-width-strong); }"],
    ["css", "/* red, 12px, #fff — в комментарии */ a { color: var(--color-accent-default); }"],
    ["css", "a { color: var(--color-gold-text); }"],
    ["script", "<p style={{ width: `var(--space-${step})`, margin: 0, opacity: 1 }} />"],
    ["script", "// white 12px #fff\nconst x = { top: offset };"],
  ];

  for (const [kind, snippet] of OK) {
    it(`допустимо: ${snippet}`, () => {
      expect(violations(snippet, kind)).toEqual([]);
    });
  }
});
