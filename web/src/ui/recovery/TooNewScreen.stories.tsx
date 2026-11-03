import type { Meta, StoryObj } from "@storybook/react-vite";
import { TooNewScreen } from "./TooNewScreen";

const meta = {
  title: "Экраны/База создана более новой версией",
  component: TooNewScreen,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof TooNewScreen>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Default: the heading and explanation centered in the window, no buttons. */
export const Обычное: Story = {};

/** English. */
export const Английский: Story = { globals: { language: "en" } };

/** Large text. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** English, large text. */
export const АнглийскийКрупный: Story = { globals: { language: "en", textScale: "large" } };
