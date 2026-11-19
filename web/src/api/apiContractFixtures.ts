import { readFileSync } from "node:fs";
import { join } from "node:path";

/** A fixture file of `fixtures/api-contract/`, the same file `crates/kinige-store/tests/api_contract/` reads. */
export function fixture<T>(name: string): T {
  const path = join(import.meta.dirname, "..", "..", "..", "fixtures", "api-contract", `${name}.json`);
  return JSON.parse(readFileSync(path, "utf8")) as T;
}

/**
 * A text written as a string or as a list of parts, where `[unit, n]` is the unit repeated `n` times and `{ cp: n }` is the one character
 * with code point `n` (for what the file must not hold itself: a bidi override).
 */
export type Spec = string | readonly (string | readonly [string, number] | { readonly cp: number })[];

export function spec(value: Spec): string {
  if (typeof value === "string") return value;
  return value
    .map((part) => (typeof part === "string" ? part : Array.isArray(part) ? part[0].repeat(part[1]) : String.fromCodePoint((part as { cp: number }).cp)))
    .join("");
}
