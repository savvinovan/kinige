import { KinigeApiError } from "./KinigeApi";
import { isClosingFence, openingFence, rustTrim } from "./mockText";

// Task items of an entry for `MockApi`: a port of `kinige_domain::tasks::set_task_item` (`fixtures/api-contract/task-items.json` pins both).
// An item is a line (after indentation and quote marks) with a list marker, a gap, `[ ]`, `[x]` or `[X]`, a gap and non-empty text; lines
// in fenced code do not count.

/** The entry text with item `index` set to `checked`; `task_item_stale` if there is no such item or it is already in that state. */
export function toggleTaskItem(bodyMd: string, index: number, checked: boolean): string {
  const lines = bodyMd.split("\n");
  let seen = 0;
  let fence: readonly [string, number] | null = null;
  for (const [number, line] of lines.entries()) {
    if (fence) {
      if (isClosingFence(line, fence[0], fence[1])) fence = null;
      continue;
    }
    fence = openingFence(line);
    if (fence) continue;
    const item = itemIn(line);
    if (!item) continue;
    if (seen++ !== index) continue;
    if (item.checked === checked) break;
    lines[number] = `${line.slice(0, item.at)}${checked ? "x" : " "}${line.slice(item.at + 1)}`;
    return lines.join("\n");
  }
  throw new KinigeApiError("task_item_stale");
}

/** The place of the character in the brackets and the state, if the line is a task item. */
function itemIn(line: string): { at: number; checked: boolean } | null {
  let i = 0;
  while (line[i] === " " || line[i] === "\t" || line[i] === ">") i += 1;
  const first = line[i] ?? "";
  if (first === "-" || first === "*" || first === "+") {
    i += 1;
  } else if (isDigit(first)) {
    let digits = 0;
    while (isDigit(line[i + digits])) digits += 1;
    const after = line[i + digits];
    if (digits > 9 || (after !== "." && after !== ")")) return null;
    i += digits + 1;
  } else {
    return null;
  }
  const gap = gapLength(line, i);
  if (gap === 0) return null;
  i += gap;
  if (line[i] !== "[" || line[i + 2] !== "]") return null;
  const mark = line[i + 1];
  if (mark !== " " && mark !== "x" && mark !== "X") return null;
  const tail = line.slice(i + 3);
  const spaces = gapLength(tail, 0);
  if (spaces === 0 || rustTrim(tail.slice(spaces)) === "") return null;
  return { at: i + 1, checked: mark !== " " };
}

function isDigit(char: string | undefined): boolean {
  return char !== undefined && char >= "0" && char <= "9";
}

/** Spaces and tabs from `from`. */
function gapLength(text: string, from: number): number {
  let length = 0;
  while (text[from + length] === " " || text[from + length] === "\t") length += 1;
  return length;
}
