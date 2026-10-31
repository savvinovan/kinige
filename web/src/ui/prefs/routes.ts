/** Страницы окна «Настройки» (слайс 4b): разделы дерева и подэкраны «Доступа ИИ-агентов». */
export type Route = "appearance" | "shortcuts" | "mirror" | "access" | "access/chats" | "access/log" | "file";

export interface TreeItem {
  route: Route;
  /** Ключ названия в файлах переводов. */
  label: `prefs.section.${"appearance" | "shortcuts" | "mirror" | "access" | "accessChats" | "accessLog" | "file"}`;
  /** 0 — раздел, 1 — вложенный пункт (подэкран). */
  level: 0 | 1;
}

/** Дерево разделов в порядке показа; вложенные пункты стоят под своим разделом. */
export const TREE: readonly TreeItem[] = [
  { route: "appearance", label: "prefs.section.appearance", level: 0 },
  { route: "shortcuts", label: "prefs.section.shortcuts", level: 0 },
  { route: "mirror", label: "prefs.section.mirror", level: 0 },
  { route: "access", label: "prefs.section.access", level: 0 },
  { route: "access/chats", label: "prefs.section.accessChats", level: 1 },
  { route: "access/log", label: "prefs.section.accessLog", level: 1 },
  { route: "file", label: "prefs.section.file", level: 0 },
];

/** Раздел, к которому относится подэкран; для раздела — он сам. */
export const parentOf = (route: Route): Route => (route === "access/chats" || route === "access/log" ? "access" : route);

/** Раздел с вложенными пунктами и его подэкраны: такие пункты дерева сворачиваются. */
export const GROUPS: Readonly<Partial<Record<Route, readonly Route[]>>> = {
  access: ["access/chats", "access/log"],
};

/** Группа, в которой лежит страница: сам раздел с вложенными пунктами или один из его подэкранов; иначе `undefined`. */
export function groupOf(route: Route): Route | undefined {
  const parent = parentOf(route);
  return GROUPS[parent] ? parent : undefined;
}
