import type { Decorator, Preview } from "@storybook/react-vite";
import "../src/app.css";

// Переключатели дизайн-системы в панели Storybook. Они только ставят атрибуты на <html>,
// все значения живут в токенах (src/ui/tokens.css, themes.css, base.css). Пустое значение — «по умолчанию».
// Палитра и шрифты не переключаются: выбраны (graphite, Inter + JetBrains Mono; ADR-0004).
const ATTRIBUTES: Record<string, string> = {
  textSize: "data-text-size",
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
  initialGlobals: { textSize: "", textScale: "", reduceMotion: "" },
  globalTypes: {
    // ВРЕМЕННО: кандидаты размера текста записей для выбора пользователем (ADR-0004, «Открыто»).
    // После выбора лишние значения удаляются вместе с токенами font.root.candidate-* и правилами в base.css.
    textSize: {
      description: "Размер текста записей (временный выбор)",
      toolbar: {
        title: "Размер текста",
        icon: "type",
        dynamicTitle: true,
        items: [
          { value: "13", title: "Текст записей 13 px" },
          { value: "", title: "Текст записей 14 px (по умолчанию)" },
          { value: "15", title: "Текст записей 15 px" },
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
