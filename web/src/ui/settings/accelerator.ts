// Сочетания клавиш: каноничная запись (так её хранит и принимает Rust) и вид для человека.
//
// Каноничная запись — `Ctrl+Alt+Shift+Cmd+<код>`: модификаторы строго в этом порядке, затем `event.code` клавиши
// (`Alt+Space`, `Alt+Shift+KeyV`). Код физической клавиши не зависит от раскладки: ⌥V работает и в русской раскладке.
// Показываются сочетания в порядке macOS: ⌃ ⌥ ⇧ ⌘ и клавиша.

export type Modifier = "Ctrl" | "Alt" | "Shift" | "Cmd";

/** Порядок модификаторов в каноничной записи и на экране (⌃ ⌥ ⇧ ⌘ — как в меню macOS). */
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

/** Подпись клавиши по `event.code`; `null` — клавиша не годится для сочетания (Escape, Enter, Tab, стрелки, модификаторы…). */
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

/** Разбор каноничной записи: какие модификаторы стоят и какая клавиша (подпись, как на капсуле). */
export function parseAccelerator(accelerator: string): { modifiers: Modifier[]; key: string | null } {
  const tokens = accelerator.split("+").filter((token) => token !== "");
  const modifiers = MODIFIERS.filter((modifier) => tokens.includes(modifier));
  const code = tokens.find((token) => !isModifier(token));
  return { modifiers, key: code === undefined ? null : (keyLabel(code) ?? code) };
}

/**
 * Каноничная запись → «капсулы» для показа: `Alt+Space` → `["⌥", "Space"]`, `Alt+Shift+KeyV` → `["⌥", "⇧", "V"]`.
 * Неизвестный код (запись из будущей версии) показывается как есть, а не пропадает.
 */
export function formatAccelerator(accelerator: string): readonly string[] {
  const { modifiers, key } = parseAccelerator(accelerator);
  return [...modifiers.map((modifier) => MODIFIER_GLYPHS[modifier]), ...(key === null ? [] : [key])];
}

type KeyEventLike = Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

/**
 * Нажатие → каноничная запись или `null`, если нажатие не годится: одиночный модификатор, клавиша вне списка
 * (Escape отменяет запись, Enter, Tab, стрелки и прочие не принимаются) или нет ни ⌃, ни ⌥, ни ⌘ — глобальное сочетание
 * на одну букву или ⇧+букву перехватывало бы обычный ввод. Функциональные клавиши F1–F12 допустимы и без модификаторов.
 * Окончательно сочетание проверяет Rust (`shortcut_invalid`).
 */
export function acceleratorFromEvent(event: KeyEventLike): string | null {
  if (keyLabel(event.code) === null) return null;
  const functionKey = /^F\d+$/.test(event.code);
  if (!functionKey && !event.ctrlKey && !event.altKey && !event.metaKey) return null;
  const pressed: Record<Modifier, boolean> = { Ctrl: event.ctrlKey, Alt: event.altKey, Shift: event.shiftKey, Cmd: event.metaKey };
  return [...MODIFIERS.filter((modifier) => pressed[modifier]), event.code].join("+");
}
