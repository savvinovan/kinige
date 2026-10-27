import type { ShortcutStatus } from "../../api/generated/ShortcutStatus";

// Сочетания для историй и тестов: умолчания слайса 2 (⌃⌥N — временно вместо ⌥Space, и ⌥⇧V), оба назначены.
export const DEFAULT_SHORTCUTS: ShortcutStatus[] = [
  { kind: "capture", accelerator: "Ctrl+Alt+KeyN", isDefault: true, state: "assigned" },
  { kind: "clipboard", accelerator: "Alt+Shift+KeyV", isDefault: true, state: "assigned" },
];
