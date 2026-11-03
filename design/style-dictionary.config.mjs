// design/tokens/**/*.json (DTCG) -> web/src/ui/tokens.css + web/src/ui/themes.css (CSS variables).
// Run from web/: `npm run tokens` (paths are relative to web/). CI checks that the generated files are not behind the tokens.
//
// The palette (design/tokens/palettes) and the font set (design/tokens/fontsets) sit in the tokens as they are.
// The active ones (design/tokens/active.json) are copied by the preprocessor into the color.* and font.family.* roles -> :root in tokens.css.
// themes.css: @font-face and Reduce Motion.
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
