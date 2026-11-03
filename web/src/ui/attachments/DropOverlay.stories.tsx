import type { Meta, StoryObj } from "@storybook/react-vite";
import "../feed/feed.css";
import { Message } from "../message/Message";
import { Frame } from "../stories/Frame";
import { DropOverlay } from "./DropOverlay";
import "./attachments-stories.css";

const at = (hour: number, minute: number) => new Date(2026, 9, 31, hour, minute);

const meta = {
  title: "Компоненты/Вложения: перетаскивание",
  component: DropOverlay,
  args: { visible: true },
  decorators: [(Story) => <Frame><Story /></Frame>],
  // A feed stand-in: the overlay lies over it and catches no pointer events.
  render: (args) => (
    <div className="attachments-feed">
      <Message markdown="Утром записал мысль про журнал" sentAt={at(9, 12)} />
      <Message markdown="Потом пошёл гулять" sentAt={at(9, 40)} />
      <Message markdown="Фото с прогулки добавлю вечером" sentAt={at(10, 5)} />
      <DropOverlay {...args} />
    </div>
  ),
} satisfies Meta<typeof DropOverlay>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A file is dragged over the window: a quiet dashed accent outline and "Drop to attach"; the feed stays readable under it. */
export const Виден: Story = {};

/** No drag: the overlay is invisible and hidden from the screen reader. */
export const Скрыт: Story = { args: { visible: false } };

/** English. */
export const Английский: Story = { globals: { language: "en" } };

/** Large text. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };
