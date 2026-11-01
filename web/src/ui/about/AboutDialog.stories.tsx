import type { Meta, StoryObj } from "@storybook/react-vite";
import { AboutDialog } from "./AboutDialog";

// Окно «О программе» (KINIG-54): название с версией сборки и одна строка о приложении; версия недоступна; en; крупный текст.
const meta = {
  title: "Экраны/О программе",
  component: AboutDialog,
  parameters: { layout: "fullscreen" },
  args: { title: "Kinige 0.5.0", onClose: () => undefined },
} satisfies Meta<typeof AboutDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Название и версия от Rust, строка о приложении. */
export const Версия: Story = {};

/** Rust не ответил: вместо версии прочерк. */
export const ВерсияНеизвестна: Story = { args: { title: "—" } };

/** Английский. */
export const En: Story = { globals: { language: "en" } };

/** Крупный текст. */
export const Крупный: Story = { globals: { textScale: "large" } };
