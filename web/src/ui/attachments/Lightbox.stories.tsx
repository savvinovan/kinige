import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import "../feed/feed.css";
import { Message } from "../message/Message";
import { Frame } from "../stories/Frame";
import { manyPhotos } from "./AttachmentsStage";
import { DemoMedia, demoPhoto } from "./demo";
import { Lightbox } from "./Lightbox";
import "./attachments-stories.css";

const noop = () => undefined;
const MEGABYTE = 1024 * 1024;

const FOUR = manyPhotos(4, "l");

const meta = {
  title: "Компоненты/Вложения: просмотр фото",
  component: Lightbox,
  args: { images: FOUR, index: 1, onIndexChange: noop, onClose: noop, onOpenExternal: noop },
  // The window behind the lightbox (1280 x 800 in the snapshots): the feed under the near-opaque scrim.
  decorators: [
    (Story) => (
      <Frame>
        <DemoMedia>
          <div className="attachments-feed">
            <Message markdown="Вид с балкона" sentAt={new Date(2026, 9, 31, 9, 12)} />
          </div>
          <Story />
        </DemoMedia>
      </Frame>
    ),
  ],
} satisfies Meta<typeof Lightbox>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The second of four photos: "2 / 4", the arrows, "Open in default app" and the cross. The photo fits the window. */
export const ВторойИзЧетырёх: Story = {};

/** The first photo: "previous" is muted and does nothing, the button stays so that focus does not jump. */
export const Первое: Story = { args: { index: 0 } };

/** The last photo: "next" is muted. */
export const Последнее: Story = { args: { index: 3 } };

/** One photo: no counter and no arrows. */
export const Одно: Story = { args: { images: [demoPhoto("solo", "IMG_0007.jpg", 1600, 900)], index: 0 } };

/** A portrait photo fits the height. */
export const Портрет: Story = { args: { images: [demoPhoto("tall", "IMG_0008.jpg", 600, 900)], index: 0 } };

/** The file is over 64 MB: the protocol serves only the thumbnail, the lightbox shows it. */
export const БольшойФайл: Story = {
  args: { images: [demoPhoto("big", "IMG_0009.jpg", 8000, 6000, { byteSize: 80 * MEGABYTE })], index: 0 },
};

/** The photo cannot be decoded: "No preview" in the same area, "Open in default app" still works. */
export const НетПревью: Story = { args: { images: [demoPhoto("bad", "IMG_0010.jpg", 1280, 960, { broken: true })], index: 0 } };

/** The file is missing from the disk. */
export const ФайлаНет: Story = { args: { images: [demoPhoto("lost", "IMG_0011.jpg", 1280, 960, { missing: true })], index: 0 } };

/** English. */
export const Английский: Story = { globals: { language: "en" } };

/** Large text: the buttons and the counter grow, the photo takes what is left. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** Alive: the arrows and Left / Right move between the photos, Escape does nothing here (the story has no window to close). */
export const Интерактивный: Story = {
  render: (args) => <Interactive {...args} />,
};

function Interactive(args: React.ComponentProps<typeof Lightbox>) {
  const [index, setIndex] = useState(args.index);
  return <Lightbox {...args} index={index} onIndexChange={setIndex} />;
}
