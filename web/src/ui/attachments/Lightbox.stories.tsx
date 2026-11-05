import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useState } from "react";
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

/** The wide photo, 1600 by 900: fits by width at fit size, by height after a quarter turn. */
const WIDE = [demoPhoto("wide", "IMG_0012.jpg", 1600, 900)];

/** 150%: the toolbar shows the percentage, "Zoom out" is available again. */
export const Увеличено: Story = {
  args: { images: WIDE, index: 0 },
  render: (args) => <Scripted keys={["+"]} {...args} />,
};

/** 300% dragged to the left and up: the pan is clamped, the cursor on the stage is the open hand. */
export const УвеличеноИСдвинуто: Story = {
  args: { images: WIDE, index: 0 },
  render: (args) => <Scripted keys={["+", "+", "+"]} drag={{ x: -420, y: 160 }} {...args} />,
};

/** A quarter turn counter-clockwise: the wide photo now fits the height. */
export const Повёрнуто: Story = {
  args: { images: WIDE, index: 0 },
  render: (args) => <Scripted keys={["r"]} {...args} />,
};

/** Turned and zoomed to 200%: zoom and pan work in the turned frame. */
export const ПовёрнутоИУвеличено: Story = {
  args: { images: WIDE, index: 0 },
  render: (args) => <Scripted keys={["r", "+", "+"]} {...args} />,
};

/** The toolbar at 800% in English: the percentage fits the button and "Zoom in" is muted. */
export const МаксимальныйМасштаб: Story = {
  args: { images: WIDE, index: 0 },
  globals: { language: "en" },
  render: (args) => <Scripted keys={["+", "+", "+", "+", "+", "+"]} {...args} />,
};

/** Large text with the viewer zoomed: the toolbar grows, the percentage keeps its width. */
export const КрупныйТекстУвеличено: Story = {
  args: { images: WIDE, index: 0 },
  globals: { textScale: "large" },
  render: (args) => <Scripted keys={["r", "+"]} {...args} />,
};

/**
 * Storybook only: presses keys and drags like a hand would, right after the viewer mounts, so that a snapshot shows the state. The
 * keys go to the document (the viewer listens there); the drag goes through the pointer events of the stage.
 */
function Scripted({ keys, drag, ...args }: React.ComponentProps<typeof Lightbox> & { keys: string[]; drag?: { x: number; y: number } }) {
  useEffect(() => {
    // After the viewer has measured its stage (its layout effect re-renders once): a task later, not in this effect.
    const timer = window.setTimeout(() => {
      for (const key of keys) document.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
      const stage = document.querySelector(".lightbox__stage");
      if (!drag || !stage) return;
      const at = (dx: number, dy: number) => ({ pointerId: 1, button: 0, bubbles: true, clientX: 640 + dx, clientY: 400 + dy });
      stage.dispatchEvent(new PointerEvent("pointerdown", at(0, 0)));
      stage.dispatchEvent(new PointerEvent("pointermove", at(drag.x, drag.y)));
      stage.dispatchEvent(new PointerEvent("pointerup", at(drag.x, drag.y)));
    }, 0);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once, on mount
  }, []);
  return <Lightbox {...args} />;
}
