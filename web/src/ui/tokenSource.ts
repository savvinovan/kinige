// Токены из design/tokens/*.json в виде, удобном для тестов и историй Storybook.
// Читаются сами JSON (а не tokens.css), поэтому видны все кандидаты, а не только активный.

interface Leaf {
  $value: unknown;
  $description?: string;
  $extensions?: { kinige?: { origin?: string } };
}

const isLeaf = (node: unknown): node is Leaf => typeof node === "object" && node !== null && "$value" in node;

/** Объединяет группу `group` из нескольких файлов токенов (каждый файл вносит свой кандидат). */
const load = (files: Record<string, unknown>, group: string): Record<string, unknown> =>
  Object.assign({}, ...Object.values(files).map((file) => (file as Record<string, unknown>)[group])) as Record<
    string,
    unknown
  >;

const paletteFiles = import.meta.glob("../../../design/tokens/palettes/*.json", { eager: true, import: "default" });
const fontsetFiles = import.meta.glob("../../../design/tokens/fontsets/*.json", { eager: true, import: "default" });
const activeFile = import.meta.glob("../../../design/tokens/active.json", { eager: true, import: "default" });

/** Роли цвета, которые должна задавать каждая палитра. */
export const PALETTE_ROLES = [
  "surface.background",
  "surface.paper",
  "text.primary",
  "text.secondary",
  "text.tertiary",
  "accent.default",
  "border.divider",
  "border.control",
  "highlight.background",
  "focus.ring",
  "status.error",
] as const;

function collectLeaves(node: unknown, path: string[], into: Map<string, Leaf>): void {
  if (isLeaf(node)) {
    into.set(path.join("."), node);
    return;
  }
  if (typeof node !== "object" || node === null) return;
  for (const [key, child] of Object.entries(node)) {
    if (!key.startsWith("$")) collectLeaves(child, [...path, key], into);
  }
}

export interface Palette {
  description: string;
  /** Роль («surface.paper») -> цвет «#rrggbb». */
  colors: Record<string, string>;
  /** Роль -> откуда цвет (официальное имя или «подгонка: …»); есть у палитр по мотивам готовых схем. */
  origins: Record<string, string>;
}

export const palettes: Record<string, Palette> = Object.fromEntries(
  Object.entries(load(paletteFiles, "palette") as Record<string, Record<string, unknown>>).map(([name, node]) => {
    const leaves = new Map<string, Leaf>();
    collectLeaves(node, [], leaves);
    return [
      name,
      {
        description: String(node.$description ?? ""),
        colors: Object.fromEntries([...leaves].map(([role, leaf]) => [role, String(leaf.$value)])),
        origins: Object.fromEntries(
          [...leaves].flatMap(([role, leaf]) => {
            const origin = leaf.$extensions?.kinige?.origin;
            return origin ? [[role, origin]] : [];
          }),
        ),
      },
    ];
  }),
);

export interface FontFace {
  family: string;
  style: "normal" | "italic";
  /** «400» или диапазон переменного шрифта «200 900». */
  weight: string;
  /** Путь относительно web/src/ui/. */
  file: string;
}

export interface Fontset {
  description: string;
  /** Роль (serif / sans / mono) -> список семейств, первое — встроенное. */
  families: Record<string, string[]>;
  /** Ключ начертания («serif-italic») -> описание. Префикс ключа — роль. */
  faces: Record<string, FontFace>;
}

export const fontsets: Record<string, Fontset> = Object.fromEntries(
  Object.entries(load(fontsetFiles, "fontset") as Record<string, Record<string, Record<string, Leaf> | string>>).map(
    ([name, node]) => [
      name,
      {
        description: String(node.$description ?? ""),
        families: Object.fromEntries(
          Object.entries(node.family as Record<string, Leaf>)
            .filter(([key]) => !key.startsWith("$"))
            .map(([role, leaf]) => [role, leaf.$value as string[]]),
        ),
        faces: Object.fromEntries(
          Object.entries(node.face as Record<string, Leaf>)
            .filter(([key]) => !key.startsWith("$"))
            .map(([key, leaf]) => [key, leaf.$value as FontFace]),
        ),
      },
    ],
  ),
);

/** Что сейчас по умолчанию (design/tokens/active.json). */
export const active = Object.fromEntries(
  Object.entries(load(activeFile, "active"))
    .filter(([key]) => !key.startsWith("$"))
    .map(([key, leaf]) => [key, String((leaf as Leaf).$value)]),
) as { palette: string; fontset: string };

const motionFile = import.meta.glob("../../../design/tokens/motion.json", { eager: true, import: "default" });

/** Длительности анимаций: обычные и для Reduce Motion (строки как в токенах, например «120ms»). */
export const motionDurations: Record<string, { normal: string; reduced: string }> = (() => {
  const motion = load(motionFile, "motion") as {
    duration: Record<string, Leaf>;
    reduced: { duration: Record<string, Leaf> };
  };
  return Object.fromEntries(
    Object.entries(motion.duration)
      .filter(([key]) => !key.startsWith("$"))
      .map(([key, leaf]) => [
        key,
        { normal: String(leaf.$value), reduced: String(motion.reduced.duration[key]?.$value) },
      ]),
  );
})();
