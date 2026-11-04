const EXTENSION_MAX = 12;
const TAIL_STEM_SYMBOLS = 4;

/**
 * A file name that is cut in the middle when it does not fit: the start shrinks with an ellipsis, the end with the extension always
 * stays visible. The full name is `title` of the whole element; the parts read together as the name.
 */
export function FileName({ name, className }: { name: string; className?: string }) {
  const { stem, tail } = splitFileName(name);
  return (
    <span className={className ? `file-name ${className}` : "file-name"} title={name}>
      <span className="file-name__stem">{stem}</span>
      <span className="file-name__tail">{tail}</span>
    </span>
  );
}

/** The file extension without the dot; empty if the name has none (a leading dot is not an extension) or it is implausibly long. */
export function fileExtension(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return "";
  const extension = name.slice(dot + 1);
  return extension.length <= EXTENSION_MAX ? extension : "";
}

/** Splits a name for the middle ellipsis: the tail is the extension and the last few symbols of the stem. */
export function splitFileName(name: string): { stem: string; tail: string } {
  const extension = fileExtension(name);
  const base = extension === "" ? name : name.slice(0, name.length - extension.length - 1);
  const symbols = Array.from(base);
  const keep = Math.min(TAIL_STEM_SYMBOLS, symbols.length);
  const stem = symbols.slice(0, symbols.length - keep).join("");
  const tail = symbols.slice(symbols.length - keep).join("") + (extension === "" ? "" : `.${extension}`);
  return { stem, tail };
}
