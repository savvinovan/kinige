import type { Decorator, Preview } from "@storybook/react-vite";
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

const preview: Preview = {
  parameters: { layout: "fullscreen" },
  decorators: [withDesignSystem],
  initialGlobals: { textScale: "", reduceMotion: "" },
  globalTypes: {
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
