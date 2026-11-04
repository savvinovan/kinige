import type { Meta, StoryObj } from "@storybook/react-vite";
import { Bubble, manyPhotos, withScene } from "./AttachmentsStage";
import { demoFile, demoPhoto } from "./demo";
import { MessageAttachments } from "./MessageAttachments";

const noop = () => undefined;
const MEGABYTE = 1024 * 1024;

const meta = {
  title: "Компоненты/Вложения: в сообщении",
  component: MessageAttachments,
  args: { attachments: [], onOpenImage: noop, onOpenFile: noop },
  decorators: [withScene],
} satisfies Meta<typeof MessageAttachments>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Two photos, a PDF, then the text and the time: the album, the file card below it, the caption. */
export const ФотоФайлИТекст: Story = {
  render: () => (
    <Bubble
      attachments={[
        demoPhoto("a", "IMG_0001.jpg", 1280, 960),
        demoFile("pdf", "Меню.pdf", "file", 2.4 * MEGABYTE),
        demoPhoto("b", "IMG_0002.jpg", 960, 1280),
      ]}
      text="Ужин в субботу: фото и меню"
    />
  ),
};

/** An entry with attachments only, no text: the bubble holds the album and the time. */
export const БезТекста: Story = {
  render: () => (
    <>
      <Bubble attachments={manyPhotos(1)} />
      <Bubble attachments={manyPhotos(3)} />
      <Bubble attachments={[demoFile("pdf", "Меню.pdf", "file", 2.4 * MEGABYTE)]} />
    </>
  ),
};

/** Only files: several cards one under another, in order. */
export const ТолькоФайлы: Story = {
  render: () => (
    <Bubble
      attachments={[
        demoFile("f1", "Меню.pdf", "file", 2.4 * MEGABYTE),
        demoFile("f2", "Прогулка.mp4", "video", 148 * MEGABYTE),
        demoFile("f3", "Заметка.mp3", "audio", 3.1 * MEGABYTE),
      ]}
      text="Три файла"
    />
  ),
};

/** English. */
export const Английский: Story = {
  globals: { language: "en" },
  render: () => (
    <Bubble
      attachments={[demoPhoto("ea", "IMG_0001.jpg", 1280, 960), demoPhoto("eb", "IMG_0002.jpg", 1280, 960), demoFile("epdf", "menu.pdf", "file", 2.4 * MEGABYTE)]}
      text="Dinner on Saturday"
    />
  ),
};

/** Large text. */
export const КрупныйТекст: Story = {
  globals: { textScale: "large" },
  render: () => <Bubble attachments={[...manyPhotos(2), demoFile("pdf", "Меню.pdf", "file", 2.4 * MEGABYTE)]} text="Ужин" />,
};
