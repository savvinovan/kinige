// Tokens from design/tokens/*.json in a form convenient for tests and Storybook stories.
// The JSON files themselves are read (not tokens.css), so all candidates are visible, not only the active one.

interface Leaf {
  $value: unknown;
  $description?: string;
  $extensions?: { kinige?: { origin?: string } };
}

const isLeaf = (node: unknown): node is Leaf => typeof node === "object" && node !== null && "$value" in node;

/** Merges the group `group` from several token files (each file contributes its own candidate). */
const load = (files: Record<string, unknown>, group: string): Record<string, unknown> =>
  Object.assign({}, ...Object.values(files).map((file) => (file as Record<string, unknown>)[group])) as Record<
    string,
    unknown
  >;

const paletteFiles = import.meta.glob("../../../design/tokens/palettes/*.json", { eager: true, import: "default" });
const fontsetFiles = import.meta.glob("../../../design/tokens/fontsets/*.json", { eager: true, import: "default" });
const activeFile = import.meta.glob("../../../design/tokens/active.json", { eager: true, import: "default" });

/** Tiles with a letter for chats without an emoji (D2): saturated muted colors, with `text.primary` text on them. */
export const TILE_ROLES = ["tile.blue", "tile.teal", "tile.green", "tile.orange", "tile.pink", "tile.violet", "tile.grey"] as const;

/** Color roles that a palette must define. */
export const PALETTE_ROLES = [
  "surface.background",
  "surface.paper",
  "surface.message",
  "surface.code",
  "surface.day",
  "surface.selected",
  "surface.hover",
  "surface.card",
  "surface.field",
  "surface.field-active",
  "text.primary",
  "text.secondary",
  "text.tertiary",
  "accent.default",
  "border.divider",
  "highlight.background",
  "highlight.row",
  "highlight.mark",
  "focus.ring",
  "status.error",
  ...TILE_ROLES,
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
  /** Role ("surface.paper") -> color "#rrggbb". */
  colors: Record<string, string>;
  /** Role -> where the color comes from (an official name or "подгонка: ..."); present for palettes based on existing schemes. */
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
  /** "400" or the range of a variable font "200 900". */
  weight: string;
  /** Path relative to web/src/ui/. */
  file: string;
}

export interface Fontset {
  description: string;
  /** Role (sans / mono / emoji) -> list of families, the first one is embedded (emoji use system fonts). */
  families: Record<string, string[]>;
  /** Face key ("sans-italic-variable") -> description. The key prefix is the role. */
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

/** What is the default right now (design/tokens/active.json). */
export const active = Object.fromEntries(
  Object.entries(load(activeFile, "active"))
    .filter(([key]) => !key.startsWith("$"))
    .map(([key, leaf]) => [key, String((leaf as Leaf).$value)]),
) as { palette: string; fontset: string };

const motionFile = import.meta.glob("../../../design/tokens/motion.json", { eager: true, import: "default" });

/** Animation durations: regular and for Reduce Motion (strings as in the tokens, for example "120ms"). */
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
