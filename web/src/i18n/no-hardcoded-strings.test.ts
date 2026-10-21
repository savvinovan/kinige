import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";

// Строки интерфейса живут только в `src/locales` (ADR-0005). Проверка идёт по дереву разбора TypeScript, а не по тексту:
// кириллица в комментариях допустима (комментарии русские), в строковых литералах, шаблонах и тексте JSX — нет.
//
// Исключения (не интерфейс): тесты и истории (их данные и ожидаемые строки), демо-данные, сгенерированные типы и
// инструменты проверки токенов, которые запускаются только в тестах и пользователю ничего не показывают.
const SRC = join(import.meta.dirname, "..");
const EXEMPT = [
  /\.test\.tsx?$/,
  /\.stories\.tsx?$/,
  /^locales\//,
  /^api\/generated\//,
  /^api\/demoData\.ts$/,
  /^test\//,
  /^ui\/(contrast|contrastPairs|fontFile|tokenSource)\.ts$/,
];
const CYRILLIC = /[А-Яа-яЁё]/;

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path);
    return /\.tsx?$/.test(name) && !EXEMPT.some((pattern) => pattern.test(relative(SRC, path).replaceAll("\\", "/"))) ? [path] : [];
  });
}

/** Строки с кириллицей в коде: `[номер строки, начало текста]`. Комментарии не в счёт. */
function cyrillicStrings(code: string, fileName: string): [number, string][] {
  const kind = fileName.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
  const file = ts.createSourceFile(fileName, code, ts.ScriptTarget.Latest, true, kind);
  const found: [number, string][] = [];
  const visit = (node: ts.Node) => {
    if (
      ts.isStringLiteral(node) ||
      ts.isNoSubstitutionTemplateLiteral(node) ||
      ts.isTemplateHead(node) ||
      ts.isTemplateMiddle(node) ||
      ts.isTemplateTail(node) ||
      ts.isJsxText(node)
    ) {
      if (CYRILLIC.test(node.text)) found.push([file.getLineAndCharacterOfPosition(node.getStart()).line + 1, node.text.trim().slice(0, 60)]);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return found;
}

describe("строки интерфейса", () => {
  it("в коде (вне тестов, историй, демо-данных и переводов) нет кириллицы в строках", () => {
    const violations = sources(SRC).flatMap((path) =>
      cyrillicStrings(readFileSync(path, "utf8"), path).map(([line, value]) => `${relative(SRC, path)}:${line}: «${value}»`),
    );
    expect(violations).toEqual([]);
  });

  it("проверка ловит строки, шаблоны и текст JSX и не трогает комментарии", () => {
    const code = ['const a = "Привет"; // комментарий по-русски', "const b = `шаблон ${a}`;", "/* и блочный */", "const c = <p>Текст</p>;"].join("\n");
    expect(cyrillicStrings(code, "x.tsx").map(([line]) => line)).toEqual([1, 2, 4]);
    expect(cyrillicStrings("// только комментарий\nconst d = 'latin';", "y.ts")).toEqual([]);
  });
});
