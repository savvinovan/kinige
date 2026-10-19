import type { Meta, StoryObj } from "@storybook/react-vite";
import { EmptyWindow } from "./EmptyWindow";
import { MockApi } from "./api/MockApi";

const meta = {
  title: "Каркас/Пустое окно",
  component: EmptyWindow,
  args: { api: new MockApi() },
  argTypes: { api: { control: false } },
} satisfies Meta<typeof EmptyWindow>;

export default meta;

export const Default: StoryObj<typeof meta> = {};
