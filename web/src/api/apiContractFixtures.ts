import { readFileSync } from "node:fs";
import { join } from "node:path";

/** A fixture file of `fixtures/api-contract/`, the same file `crates/kinige-store/tests/api_contract/` reads. */
export function fixture<T>(name: string): T {
  const path = join(import.meta.dirname, "..", "..", "..", "fixtures", "api-contract", `${name}.json`);
  return JSON.parse(readFileSync(path, "utf8")) as T;
}

/** A text written as a string or as a list of parts, where `[unit, n]` is the unit repeated `n` times. */
export type Spec = string | readonly (string | readonly [string, number])[];

export function spec(value: Spec): string {
  if (typeof value === "string") return value;
  return value.map((part) => (typeof part === "string" ? part : part[0].repeat(part[1]))).join("");
}
