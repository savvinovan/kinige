import type { Meta, StoryObj } from "@storybook/react-vite";
import { AboutDialog } from "./AboutDialog";

// The "About" window (KINIG-54): the name with the build version and one line about the app; version unavailable; en; large text.
const meta = {
  title: "Экраны/О программе",
  component: AboutDialog,
  parameters: { layout: "fullscreen" },
  args: { title: "Kinige 0.5.0", onClose: () => undefined },
} satisfies Meta<typeof AboutDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The name and version from Rust, a line about the app. */
export const Версия: Story = {};

/** Rust did not answer: a dash instead of the version. */
export const ВерсияНеизвестна: Story = { args: { title: "—" } };

/** English. */
export const En: Story = { globals: { language: "en" } };

/** Large text. */
export const Крупный: Story = { globals: { textScale: "large" } };
