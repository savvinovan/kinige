import type { Decorator, Preview } from "@storybook/react-vite";
import "../src/app.css";
import { palettes } from "../src/ui/tokenSource";

// Переключатели дизайн-системы в панели Storybook. Они только ставят атрибуты на <html>,
// все значения живут в токенах (src/ui/tokens.css, themes.css, base.css). Пустое значение — «по умолчанию» из design/tokens/active.json.
// Шрифты не переключаются: набор выбран (design/tokens/fontsets/literata.json).
const ATTRIBUTES: Record<string, string> = {
  palette: "data-palette",
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

const preview: Preview = {
  parameters: { layout: "fullscreen" },
  decorators: [withDesignSystem],
  initialGlobals: { palette: "", textScale: "", reduceMotion: "" },
  globalTypes: {
    palette: {
      description: "Кандидат палитры",
      toolbar: {
        title: "Палитра",
        icon: "paintbrush",
        dynamicTitle: true,
        items: [
          { value: "", title: "Палитра: по умолчанию" },
          ...Object.keys(palettes).map((name) => ({ value: name, title: name })),
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
