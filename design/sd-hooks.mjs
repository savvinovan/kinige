// Style Dictionary hooks for the Kinige design system: choosing the active candidates, the core filter, the themes.css format.

/** Copies the active palette and font candidates into the color.* and font.family.* roles. */
export function selectActive(tokens) {
  const next = structuredClone(tokens);
  const pick = (group, key) => {
    const name = next.active?.[key]?.$value;
    const candidate = next[group]?.[name];
    if (!candidate) {
      const known = Object.keys(next[group] ?? {}).join(", ");
      throw new Error(`design/tokens/active.json: ${key} = "${name}" not found in ${group} (known: ${known})`);
    }
    return structuredClone(candidate);
  };
  // The palette supplies the roles; other `color.*` groups from their own files (`forest.json`, slice 6) stay.
  next.color = { ...pick("palette", "palette"), ...next.color };
  next.font = { ...next.font, family: pick("fontset", "fontset").family };
  return next;
}

const CANDIDATE_ROOTS = new Set(["palette", "fontset", "active"]);

/** tokens.css gets everything except the candidates, the choice and the Reduce Motion values. */
export function coreFilter(token) {
  const [root, second] = token.path;
  return !CANDIDATE_ROOTS.has(root) && !(root === "motion" && second === "reduced");
}

const declarations = (pairs, indent = "  ") => pairs.map(([name, value]) => `${indent}--${name}: ${value};`).join("\n");

/** themes.css: @font-face, Reduce Motion. */
export function candidatesFormat({ dictionary }) {
  const tokens = dictionary.allTokens;
  const out = ["/* Generated from design/tokens (npm run tokens). Do not edit by hand. */", ""];

  out.push("/* Font faces: paths are relative to this file. */");
  for (const face of tokens.filter((t) => t.path[0] === "fontset" && t.path[2] === "face")) {
    const { family, style, weight, file } = face.$value;
    out.push(
      "@font-face {",
      `  font-family: "${family}";`,
      `  font-style: ${style};`,
      `  font-weight: ${weight};`,
      "  font-display: block;",
      `  src: url("./${file}") format("woff2");`,
      "}",
      "",
    );
  }

  const reduced = tokens
    .filter((t) => t.path[0] === "motion" && t.path[1] === "reduced")
    .map((t) => [`motion-${t.path.slice(2).join("-")}`, t.$value]);
  out.push(
    "/* Reduce Motion: the OS setting or the data-reduce-motion attribute on any element (the switch in Storybook and in the settings). */",
    "@media (prefers-reduced-motion: reduce) {",
    "  :root {",
    declarations(reduced, "    "),
    "  }",
    "}",
    "",
    "[data-reduce-motion] {",
    declarations(reduced),
    "}",
    "",
  );
  return out.join("\n");
}
