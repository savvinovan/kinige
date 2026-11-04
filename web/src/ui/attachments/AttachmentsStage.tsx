import type { Decorator } from "@storybook/react-vite";
import type { ReactNode } from "react";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import "../feed/feed.css";
import { Message } from "../message/Message";
import { Frame } from "../stories/Frame";
import { DemoMedia, demoPhoto } from "./demo";
import { MessageAttachments } from "./MessageAttachments";
import "./attachments-stories.css";

const noop = () => undefined;

// The time is fixed so that snapshots do not change from run to run.
const SENT_AT = new Date(2026, 9, 31, 9, 12);

// Width and height of the photos in `manyPhotos`: landscape, portrait, wide, square, tall.
const SIZES = [
  [1280, 960],
  [960, 1280],
  [1600, 900],
  [1024, 1024],
  [800, 1200],
] as const;

/** Storybook only: the paper backdrop, the synthetic pictures and a column of the feed's width (or a 640 px window). */
export function Scene({ children, narrow = false }: { children: ReactNode; narrow?: boolean }) {
  return (
    <Frame>
      <DemoMedia>
        <div className={narrow ? "attachments-scene attachments-scene--narrow" : "attachments-scene"}>{children}</div>
      </DemoMedia>
    </Frame>
  );
}

/** Storybook only: a bubble with attachments above its text (the integration into `Message` is done by the feed). */
export function Bubble({ attachments, text = "" }: { attachments: readonly AttachmentView[]; text?: string }) {
  return (
    <Message
      markdown={text}
      sentAt={SENT_AT}
      quote={
        <div className="attachments-scene__slot">
          <MessageAttachments attachments={attachments} onOpenImage={noop} onOpenFile={noop} />
        </div>
      }
    />
  );
}

/** Storybook only: `count` photos of mixed shapes named IMG_0001.jpg, IMG_0002.jpg, and so on. */
export function manyPhotos(count: number, prefix = "p"): AttachmentView[] {
  return Array.from({ length: count }, (_, i) => {
    const [width, height] = SIZES[i % SIZES.length] ?? SIZES[0];
    return demoPhoto(`${prefix}${i}`, `IMG_${String(i + 1).padStart(4, "0")}.jpg`, width, height);
  });
}

/** Storybook only: the decorator of the attachment stories; `parameters: { narrow: true }` gives the 640 px window. */
export const withScene: Decorator = (Story, context) => (
  <Scene narrow={context.parameters.narrow === true}>
    <Story />
  </Scene>
);
