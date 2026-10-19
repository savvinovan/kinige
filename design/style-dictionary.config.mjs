// design/tokens/*.json (DTCG) -> web/src/ui/tokens.css (CSS-переменные в :root).
// Запуск из web/: `npm run tokens` (пути относительно web/). CI проверяет, что tokens.css не отстал от токенов.
export default {
  source: ["../design/tokens/**/*.json"],
  platforms: {
    css: {
      transformGroup: "css",
      buildPath: "src/ui/",
      files: [
        {
          destination: "tokens.css",
          format: "css/variables",
          options: { outputReferences: true, showFileHeader: false },
        },
      ],
    },
  },
};
