import type { Meta, StoryObj } from "@storybook/react-vite";
import { Bubble, manyPhotos, withScene } from "./AttachmentsStage";
import { demoPhoto } from "./demo";

// A GIF in the feed (slice 5c). The synthetic "GIF" is a picture; its `full` says "playing" so a story can tell it from the first frame. A GIF plays while
// at least half of its cell is on screen: in Storybook that is the case for these stories, except where the size rules or Reduce Motion hold it still.

const MIB = 1024 * 1024;
const gif = (id: string, byteSize: number, width = 480, height = 270, options: Parameters<typeof demoPhoto>[4] = {}) =>
  demoPhoto(id, "cat.gif", width, height, { mime: "image/gif", byteSize, ...options });

const meta = {
  title: "Компоненты/Вложения: GIF",
  decorators: [withScene],
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** Plays: at most 8 MiB and 2,000,000 pixels, on screen. No badge; a click opens the viewer. */
export const Играет: Story = { render: () => <Bubble attachments={[gif("p1", 2 * MIB, 1600, 900)]} /> };

/** A GIF over 8 MiB shows its first frame with the GIF badge and plays on a click until it leaves the screen. */
export const БольшойПоРазмеруФайла: Story = { render: () => <Bubble attachments={[gif("b1", 12 * MIB, 1600, 900)]} /> };

/** A GIF over 2,000,000 pixels is held back the same way. */
export const БольшойПоПикселям: Story = { render: () => <Bubble attachments={[gif("b2", 3 * MIB, 2400, 1350)]} /> };

/** Over 64 MiB no `full` is served: the badge stays and a click opens the viewer (with "Open in default app"). */
export const БольшеШестидесятиЧетырёхМиБ: Story = { render: () => <Bubble attachments={[gif("b3", 70 * MIB, 1600, 900)]} /> };

/** Reduce Motion (the toolbar switch or the OS setting): every GIF stands still with the badge; a click plays that one. */
export const ReduceMotion: Story = {
  render: () => <Bubble attachments={[gif("r1", 2 * MIB, 1600, 900)]} />,
  globals: { reduceMotion: "on" },
};

/** The badge at the top left leaves the time pill at the bottom right alone. */
export const БейджИВремя: Story = { render: () => <Bubble attachments={[gif("t1", 20 * MIB, 1600, 900)]} extra={{ favorite: true }} /> };

/** A GIF with text under it: edge to edge at the top, the text under it. */
export const СТекстом: Story = { render: () => <Bubble attachments={[gif("x1", 20 * MIB, 1600, 900)]} text="Наш кот, снова" /> };

/** A GIF in an album with photos: its cell is a cell like the others, with the badge. */
export const ВАльбоме: Story = {
  render: () => <Bubble attachments={[gif("a1", 20 * MIB, 1600, 900), ...manyPhotos(2, "ap")]} />,
};

/** A covered GIF is the cover until it is revealed: nothing is loaded behind it. */
export const Закрытый: Story = { render: () => <Bubble attachments={[gif("c1", 2 * MIB, 1600, 900, { spoiler: true })]} /> };

/** A GIF whose file is missing keeps its box. */
export const БезФайла: Story = { render: () => <Bubble attachments={[gif("m1", 2 * MIB, 1600, 900, { missing: true })]} /> };

/** In the narrow window. */
export const УзкоеОкно: Story = { render: () => <Bubble attachments={[gif("n1", 20 * MIB, 1600, 900)]} />, parameters: { narrow: true } };

export const Английский: Story = { render: () => <Bubble attachments={[gif("e1", 20 * MIB, 1600, 900)]} />, globals: { language: "en" } };

export const КрупныйТекст: Story = {
  render: () => <Bubble attachments={[gif("l1", 20 * MIB, 1600, 900)]} text="Наш кот, снова" />,
  globals: { textScale: "large" },
};
