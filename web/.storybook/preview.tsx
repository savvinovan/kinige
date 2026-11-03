import type { Decorator, Preview } from "@storybook/react-vite";
import { I18n } from "../src/i18n/I18n";
import type { Language } from "../src/i18n";
import "../src/app.css";

// Design system switches in the Storybook toolbar. They only set attributes on <html>,
// all values live in the tokens (src/ui/tokens.css, themes.css, base.css). An empty value means "default".
// The palette and fonts are not switchable: they are chosen (graphite, Inter + JetBrains Mono; ADR-0004).
const ATTRIBUTES: Record<string, string> = {
  textScale: "data-text-scale",
  reduceMotion: "data-reduce-motion",
};

const withDesignSystem: Decorator = (Story, context) => {
  const root = document.documentElement;
  for (const [global, attribute] of Object.entries(ATTRIBUTES)) {
    const value = String(context.globals[global] ?? "");
    if (value === "") root.removeAttribute(attribute);
    else root.setAttribute(attribute, global === "reduceMotion" ? "" : value);
  }
  return Story();
};

// The interface language. Russian by default: the D1 story snapshots were taken in Russian. English is `globals: { language: "en" }`
// in a story or the toolbar switch.
const withLanguage: Decorator = (Story, context) => {
  const language: Language = context.globals.language === "en" ? "en" : "ru";
  document.documentElement.lang = language;
  return (
    <I18n language={language}>
      <Story />
    </I18n>
  );
};

// Fonts load lazily, when the text is already laid out, and a late load in the middle of the first render changes the
// anti-aliasing of rounded edges in already rasterized tiles: the capture window snapshot (with a code block in
// JetBrains Mono) sometimes matched the baseline and sometimes differed by ±1 level in the corners. All faces from
// @font-face are loaded before the story is rendered.
const loadFonts = async () => {
  await Promise.all([...document.fonts].map((face) => face.load()));
};

const preview: Preview = {
  parameters: { layout: "fullscreen" },
  loaders: [loadFonts],
  decorators: [withLanguage, withDesignSystem],
  initialGlobals: { textScale: "", reduceMotion: "", language: "ru" },
  globalTypes: {
    language: {
      description: "Язык интерфейса",
      toolbar: {
        title: "Язык",
        icon: "globe",
        dynamicTitle: true,
        items: [
          { value: "ru", title: "Русский" },
          { value: "en", title: "English" },
        ],
      },
    },
    textScale: {
      description: "Масштаб текста",
      toolbar: {
        title: "Масштаб текста",
        icon: "zoom",
        dynamicTitle: true,
        items: [
          { value: "", title: "Обычный текст" },
          { value: "large", title: "Крупный текст" },
        ],
      },
    },
    reduceMotion: {
      description: "Reduce Motion",
      toolbar: {
        title: "Движение",
        icon: "play",
        dynamicTitle: true,
        items: [
          { value: "", title: "Анимации включены" },
          { value: "on", title: "Reduce Motion" },
        ],
      },
    },
  },
};

export default preview;
