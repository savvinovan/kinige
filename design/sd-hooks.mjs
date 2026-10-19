// Хуки Style Dictionary для дизайн-системы Kinige: выбор активных кандидатов, фильтр ядра, формат themes.css.

/** Копирует активный кандидат палитры и шрифтов в роли color.* и font.family.*. */
export function selectActive(tokens) {
  const next = structuredClone(tokens);
  const pick = (group, key) => {
    const name = next.active?.[key]?.$value;
    const candidate = next[group]?.[name];
    if (!candidate) {
      const known = Object.keys(next[group] ?? {}).join(", ");
      throw new Error(`design/tokens/active.json: ${key} = "${name}" не найден в ${group} (есть: ${known})`);
    }
    return structuredClone(candidate);
  };
  next.color = pick("palette", "palette");
  next.font = { ...next.font, family: pick("fontset", "fontset").family };
  return next;
}

const CANDIDATE_ROOTS = new Set(["palette", "fontset", "active"]);

/** В tokens.css попадает всё, кроме кандидатов, выбора и значений Reduce Motion. */
export function coreFilter(token) {
  const [root, second] = token.path;
  return !CANDIDATE_ROOTS.has(root) && !(root === "motion" && second === "reduced");
}

const declarations = (pairs, indent = "  ") => pairs.map(([name, value]) => `${indent}--${name}: ${value};`).join("\n");

/** themes.css: блоки кандидатов палитр, @font-face, Reduce Motion. */
export function candidatesFormat({ dictionary }) {
  const tokens = dictionary.allTokens;
  const out = ["/* Сгенерировано из design/tokens (npm run tokens). Не править вручную. */", ""];

  out.push("/* Кандидаты палитр: [data-palette] на любом элементе, в т.ч. <html>. Блоки идут после :root из tokens.css. */");
  for (const name of unique(tokens.filter((t) => t.path[0] === "palette").map((t) => t.path[1]))) {
    const own = tokens.filter((t) => t.path[0] === "palette" && t.path[1] === name);
    out.push(`[data-palette="${name}"] {`, declarations(own.map((t) => [`color-${t.path.slice(2).join("-")}`, t.$value])), "}", "");
  }

  out.push("/* Начертания: пути относительно этого файла. */");
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
    "/* Reduce Motion: настройка ОС или атрибут data-reduce-motion на любом элементе (переключатель в Storybook и в настройках). */",
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

function unique(list) {
  return [...new Set(list)];
}
