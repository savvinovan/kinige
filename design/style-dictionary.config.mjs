// design/tokens/**/*.json (DTCG) -> web/src/ui/tokens.css + web/src/ui/themes.css (CSS-переменные).
// Запуск из web/: `npm run tokens` (пути относительно web/). CI проверяет, что сгенерированные файлы не отстали от токенов.
//
// Палитра (design/tokens/palettes) и набор шрифтов (design/tokens/fontsets) лежат в токенах как есть.
// Активные (design/tokens/active.json) препроцессором копируются в роли color.* и font.family.* -> :root в tokens.css.
// themes.css: @font-face и Reduce Motion.
import { candidatesFormat, coreFilter, selectActive } from "./sd-hooks.mjs";

export default {
  source: ["../design/tokens/**/*.json"],
  hooks: {
    preprocessors: { "kinige/select-active": selectActive },
    filters: { "kinige/core": coreFilter },
    formats: { "kinige/css-themes": candidatesFormat },
  },
  preprocessors: ["kinige/select-active"],
  platforms: {
    css: {
      transformGroup: "css",
      buildPath: "src/ui/",
      files: [
        {
          destination: "tokens.css",
          format: "css/variables",
          filter: "kinige/core",
          options: { outputReferences: true, showFileHeader: false },
        },
        {
          destination: "themes.css",
          format: "kinige/css-themes",
          options: { showFileHeader: false },
        },
      ],
    },
  },
};
