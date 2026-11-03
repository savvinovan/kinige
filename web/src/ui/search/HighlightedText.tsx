import { Fragment, useMemo } from "react";

const WORD = /[\p{L}\p{N}]+/gu;

/**
 * Text with highlighted words. `terms` are the entry's words as they are written in it (Rust provides them): only whole
 * words match, case-sensitively, so "книга" is not highlighted inside "книгами". A word is a run of letters and digits of any alphabet.
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
