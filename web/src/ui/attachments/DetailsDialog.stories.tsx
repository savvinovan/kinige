import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { DemoMedia, demoFile, demoPhoto } from "./demo";
import { DetailsDialog } from "./DetailsDialog";
import { RevealProvider } from "./reveal";

// KINIG-136: "Attachment details": the picture (covered when hidden), the name with its fixed extension, the description with a counter from 900
// characters, "Hide as spoiler" for a photo, and "2 / 4" with the arrows for an entry with several attachments.

const MEGABYTE = 1024 * 1024;
const noop = () => Promise.resolve();

const meta = {
  title: "Компоненты/Вложения: свойства вложения",
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

function Stage({
  attachments,
  error,
  revealed = [],
  start = 0,
}: {
  attachments: AttachmentView[];
  error?: string;
  revealed?: string[];
  start?: number;
}) {
  const [index, setIndex] = useState(start);
  return (
    <DemoMedia>
      <RevealProvider viewKey="story" initial={revealed}>
        <DetailsDialog attachments={attachments} index={index} onIndexChange={setIndex} onSave={noop} onClose={() => undefined} error={error} />
      </RevealProvider>
    </DemoMedia>
  );
}

const PHOTO = demoPhoto("d1", "IMG_0001.jpg", 1280, 960, { name: "beach.jpg", description: "Пляж, июль" });
const HIDDEN = demoPhoto("d2", "IMG_0002.jpg", 960, 1280, { name: "sunset.jpg", description: "Закат над заливом", spoiler: true });
const PDF = demoFile("d3", "scan0042.pdf", "file", 2.4 * MEGABYTE, "local", { name: "Договор.pdf", description: "Подписанный экземпляр" });

/** A photo with a name and a description. */
export const Photo: Story = { render: () => <Stage attachments={[PHOTO]} /> };

/** A hidden photo: the cover and a "Show" button instead of the picture; the box is ticked. */
export const HiddenPhoto: Story = { render: () => <Stage attachments={[HIDDEN]} /> };

/** A hidden photo after "Show": the same state as in the feed, the picture is there. */
export const HiddenPhotoShown: Story = { render: () => <Stage attachments={[HIDDEN]} revealed={["d2"]} /> };

/** A file card: a name and a description, and no checkbox. */
export const FileCard: Story = { render: () => <Stage attachments={[PDF]} /> };

/** An entry with several attachments: "2 / 3" and the arrows. */
export const SeveralAttachments: Story = { render: () => <Stage attachments={[PHOTO, HIDDEN, PDF]} start={1} /> };

/** A description of 950 characters: the counter shows from 900. */
export const LongDescription: Story = {
  render: () => <Stage attachments={[demoPhoto("d4", "IMG_0004.jpg", 1280, 960, { description: "Длинное описание. ".repeat(53) })]} />,
};

/** Rust refused the save: the error under the fields, the dialog stays open. */
export const SaveError: Story = {
  render: () => <Stage attachments={[PHOTO]} error="Такое название или описание не подходят. Ничего не изменено." />,
};

/** English. */
export const English: Story = { globals: { language: "en" }, render: () => <Stage attachments={[HIDDEN]} /> };

/** Large text. */
export const LargeText: Story = { globals: { textScale: "large" }, render: () => <Stage attachments={[PHOTO, PDF]} /> };
