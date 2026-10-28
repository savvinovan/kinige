import { Fragment, useMemo } from "react";

const WORD = /[\p{L}\p{N}]+/gu;

/**
 * Текст с подсвеченными словами. `terms` — слова записи, как они в ней написаны (их отдаёт Rust): совпадают только целые
 * слова и с учётом регистра, поэтому «книга» не подсвечивается внутри «книгами». Слово — серия букв и цифр любого алфавита.
 */
export function HighlightedText({ text, terms }: { text: string; terms: readonly string[] }) {
  const parts = useMemo(() => {
    const wanted = new Set(terms);
    const found: { text: string; mark: boolean }[] = [];
    let from = 0;
    for (const match of text.matchAll(WORD)) {
      if (!wanted.has(match[0])) continue;
      if (match.index > from) found.push({ text: text.slice(from, match.index), mark: false });
      found.push({ text: match[0], mark: true });
      from = match.index + match[0].length;
    }
    if (from < text.length) found.push({ text: text.slice(from), mark: false });
    return found;
  }, [text, terms]);
  return (
    <>
      {parts.map((part, index) =>
        part.mark ? (
          <mark className="search-mark" key={index}>
            {part.text}
          </mark>
        ) : (
          <Fragment key={index}>{part.text}</Fragment>
        ),
      )}
    </>
  );
}
