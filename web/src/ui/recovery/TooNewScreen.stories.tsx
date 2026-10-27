import type { Meta, StoryObj } from "@storybook/react-vite";
import { TooNewScreen } from "./TooNewScreen";

const meta = {
  title: "Экраны/База создана более новой версией",
  component: TooNewScreen,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof TooNewScreen>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Обычное: заголовок и пояснение по центру окна, кнопок нет. */
export const Обычное: Story = {};

/** Английский. */
export const Английский: Story = { globals: { language: "en" } };

/** Крупный текст. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** Английский, крупный текст. */
export const АнглийскийКрупный: Story = { globals: { language: "en", textScale: "large" } };
