import type { Decorator, Preview } from "@storybook/react-vite";
import { I18n } from "../src/i18n/I18n";
import type { Language } from "../src/i18n";
import "../src/app.css";

// Переключатели дизайн-системы в панели Storybook. Они только ставят атрибуты на <html>,
// все значения живут в токенах (src/ui/tokens.css, themes.css, base.css). Пустое значение — «по умолчанию».
// Палитра и шрифты не переключаются: выбраны (graphite, Inter + JetBrains Mono; ADR-0004).
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

// Язык интерфейса. По умолчанию русский: снимки историй D1 сняты на русском. Английский — `globals: { language: "en" }`
// в истории или переключатель в панели.
const withLanguage: Decorator = (Story, context) => {
  const language: Language = context.globals.language === "en" ? "en" : "ru";
  document.documentElement.lang = language;
  return (
    <I18n language={language}>
      <Story />
    </I18n>
  );
};

// Шрифты грузятся лениво, когда текст уже разложен, и догрузка посреди первой отрисовки меняет сглаживание скруглённых
// краёв в уже растрированных плитках: снимок окна захвата (там блок кода в JetBrains Mono) то совпадал с эталоном, то
// расходился на ±1 уровень в углах. Все начертания из @font-face загружаются до отрисовки истории.
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
