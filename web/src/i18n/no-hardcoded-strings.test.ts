import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";

// UI strings live only in `src/locales` (ADR-0005). The check walks the TypeScript syntax tree, not the text:
// Cyrillic in comments is allowed, in string literals, templates and JSX text it is not.
//
// Exceptions (not the UI): tests and stories (their data and expected strings), demo data (including the "В этот день"
// cards), generated types and the token check tools, which run only in tests and show nothing to the user.
const SRC = join(import.meta.dirname, "..");
const EXEMPT = [
  /\.test\.tsx?$/,
  /\.stories\.tsx?$/,
  /^locales\//,
  /^api\/generated\//,
  /^api\/demoData\.ts$/,
  /^ui\/onthisday\/demoCard\.ts$/,
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

/** Strings with Cyrillic in code: `[line number, start of the text]`. Comments do not count. */
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
