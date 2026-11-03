// Keyboard shortcuts: the canonical form (how Rust stores and accepts it) and the human-readable form.
//
// The canonical form is `Ctrl+Alt+Shift+Cmd+<code>`: modifiers strictly in this order, then the key's `event.code`
// (`Alt+Space`, `Alt+Shift+KeyV`). The physical key code does not depend on the layout: ⌥V works in the Russian layout too.
// Shortcuts are displayed in macOS order: ⌃ ⌥ ⇧ ⌘ and the key.

export type Modifier = "Ctrl" | "Alt" | "Shift" | "Cmd";

/** The order of modifiers in the canonical form and on screen (⌃ ⌥ ⇧ ⌘, as in macOS menus). */
export const MODIFIERS: readonly Modifier[] = ["Ctrl", "Alt", "Shift", "Cmd"];

export const MODIFIER_GLYPHS: Record<Modifier, string> = { Ctrl: "⌃", Alt: "⌥", Shift: "⇧", Cmd: "⌘" };

const PUNCTUATION: Record<string, string> = {
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
};

/** The key label by `event.code`; `null` means the key is not suitable for a shortcut (Escape, Enter, Tab, arrows, modifiers...). */
export function keyLabel(code: string): string | null {
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) return letter[1] ?? null;
  const digit = /^Digit([0-9])$/.exec(code);
  if (digit) return digit[1] ?? null;
  if (/^F(?:[1-9]|1[0-2])$/.test(code)) return code;
  if (code === "Space") return "Space";
  return PUNCTUATION[code] ?? null;
}

const isModifier = (token: string): token is Modifier => (MODIFIERS as readonly string[]).includes(token);

/** Parsing the canonical form: which modifiers are set and which key (the label, as on the capsule). */
export function parseAccelerator(accelerator: string): { modifiers: Modifier[]; key: string | null } {
  const tokens = accelerator.split("+").filter((token) => token !== "");
  const modifiers = MODIFIERS.filter((modifier) => tokens.includes(modifier));
  const code = tokens.find((token) => !isModifier(token));
  return { modifiers, key: code === undefined ? null : (keyLabel(code) ?? code) };
}

/**
 * The canonical form -> "capsules" for display: `Alt+Space` -> `["⌥", "Space"]`, `Alt+Shift+KeyV` -> `["⌥", "⇧", "V"]`.
 * An unknown code (a record from a future version) is shown as is, not dropped.
 */
export function formatAccelerator(accelerator: string): readonly string[] {
  const { modifiers, key } = parseAccelerator(accelerator);
  return [...modifiers.map((modifier) => MODIFIER_GLYPHS[modifier]), ...(key === null ? [] : [key])];
}

type KeyEventLike = Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

/**
 * A key press -> the canonical form, or `null` if the press is not suitable: a lone modifier, a key outside the list
 * (Escape cancels the recording; Enter, Tab, arrows and others are not accepted) or none of ⌃, ⌥, ⌘ present: a global shortcut
 * on a single letter or ⇧+letter would intercept ordinary typing. Function keys F1-F12 are allowed even without modifiers.
 * Rust makes the final check of the shortcut (`shortcut_invalid`).
 */
export function acceleratorFromEvent(event: KeyEventLike): string | null {
  if (keyLabel(event.code) === null) return null;
  const functionKey = /^F\d+$/.test(event.code);
  if (!functionKey && !event.ctrlKey && !event.altKey && !event.metaKey) return null;
  const pressed: Record<Modifier, boolean> = { Ctrl: event.ctrlKey, Alt: event.altKey, Shift: event.shiftKey, Cmd: event.metaKey };
  return [...MODIFIERS.filter((modifier) => pressed[modifier]), event.code].join("+");
}
