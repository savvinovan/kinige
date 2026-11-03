import type { ShortcutStatus } from "../../api/generated/ShortcutStatus";

// Shortcuts for stories and tests: the slice 2 defaults (⌃⌥N, temporarily instead of ⌥Space, and ⌥⇧V), both assigned.
export const DEFAULT_SHORTCUTS: ShortcutStatus[] = [
  { kind: "capture", accelerator: "Ctrl+Alt+KeyN", isDefault: true, state: "assigned" },
  { kind: "clipboard", accelerator: "Alt+Shift+KeyV", isDefault: true, state: "assigned" },
];
