/** Pages of the "Settings" window (slice 4b): the tree sections and the subscreens of "AI agent access". */
export type Route = "appearance" | "shortcuts" | "mirror" | "access" | "access/chats" | "access/log" | "file";

export interface TreeItem {
  route: Route;
  /** The name key in the translation files. */
  label: `prefs.section.${"appearance" | "shortcuts" | "mirror" | "access" | "accessChats" | "accessLog" | "file"}`;
  /** 0 is a section, 1 is a nested item (subscreen). */
  level: 0 | 1;
}

/** The tree of sections in display order; nested items stand under their section. */
export const TREE: readonly TreeItem[] = [
  { route: "appearance", label: "prefs.section.appearance", level: 0 },
  { route: "shortcuts", label: "prefs.section.shortcuts", level: 0 },
  { route: "mirror", label: "prefs.section.mirror", level: 0 },
  { route: "access", label: "prefs.section.access", level: 0 },
  { route: "access/chats", label: "prefs.section.accessChats", level: 1 },
  { route: "access/log", label: "prefs.section.accessLog", level: 1 },
  { route: "file", label: "prefs.section.file", level: 0 },
];

/** The section a subscreen belongs to; for a section, itself. */
export const parentOf = (route: Route): Route => (route === "access/chats" || route === "access/log" ? "access" : route);

/** A section with nested items and its subscreens: such tree items collapse. */
export const GROUPS: Readonly<Partial<Record<Route, readonly Route[]>>> = {
  access: ["access/chats", "access/log"],
};

/** The group the page belongs to: a section with nested items itself or one of its subscreens; otherwise `undefined`. */
export function groupOf(route: Route): Route | undefined {
  const parent = parentOf(route);
  return GROUPS[parent] ? parent : undefined;
}
