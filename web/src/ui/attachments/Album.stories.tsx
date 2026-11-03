import type { Meta, StoryObj } from "@storybook/react-vite";
import { Album } from "./Album";
import { Bubble, manyPhotos, withScene } from "./AttachmentsStage";
import { demoPhoto } from "./demo";

const noop = () => undefined;

const meta = {
  title: "Компоненты/Вложения: альбом",
  component: Album,
  args: { images: manyPhotos(1), onOpen: noop },
  decorators: [withScene],
  // The album sits in a bubble, as in the feed.
  render: (args) => <Bubble attachments={args.images} text="Вид с балкона" />,
} satisfies Meta<typeof Album>;

export default meta;
type Story = StoryObj<typeof meta>;

/** One photo, 1280 x 960: the box has the photo's shape from the first frame. */
export const Одно: Story = {};

/** A portrait photo (600 x 900 after the EXIF rotation): the 400 px height cap narrows the box. */
export const Портрет: Story = { args: { images: [demoPhoto("portrait", "IMG_0042.jpg", 600, 900)] } };

/** A very wide panorama: the box is as wide as the bubble and low; the photo is not cropped. */
export const Панорама: Story = { args: { images: [demoPhoto("pano", "PANO_0001.jpg", 4000, 800)] } };

/** A small picture is not enlarged beyond its own size. */
export const Маленькое: Story = { args: { images: [demoPhoto("small", "icon.png", 160, 120)] } };

/** A transparent PNG shows on the bubble background: no black box behind it. */
export const ПрозрачныйPNG: Story = { args: { images: [demoPhoto("alpha", "logo.png", 800, 600, { transparent: true })] } };

/** Two photos: one row of two cells. */
export const Два: Story = { args: { images: manyPhotos(2) } };

/** Three photos: one wide photo and two under it. */
export const Три: Story = { args: { images: manyPhotos(3) } };

/** Four photos: 2 x 2. */
export const Четыре: Story = { args: { images: manyPhotos(4) } };

/** Five photos: a row of two and a row of three. */
export const Пять: Story = { args: { images: manyPhotos(5) } };

/** Ten photos (the most one entry holds): rows of 2, 2, 3 and 3; every cell is reachable. */
export const Десять: Story = { args: { images: manyPhotos(10) } };

/** The thumbnail cannot be decoded: a tile of the same size says "No preview"; the box did not change. */
export const НетПревью: Story = {
  args: { images: [demoPhoto("broken", "IMG_0100.jpg", 1280, 960, { broken: true })] },
};

/** The file is missing from the disk: "File is missing" in the photo's box. In a grid the tile has the size of a cell. */
export const ФайлаНет: Story = {
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("gone", "IMG_0101.jpg", 1280, 960, { missing: true })]} text="Одно фото" />
      <Bubble
        attachments={[
          demoPhoto("g1", "IMG_0102.jpg", 1280, 960),
          demoPhoto("g2", "IMG_0103.jpg", 1280, 960, { missing: true }),
          demoPhoto("g3", "IMG_0104.jpg", 1280, 960, { broken: true }),
          demoPhoto("g4", "IMG_0105.jpg", 1280, 960),
        ]}
        text="Четыре, два без картинки"
      />
    </>
  ),
};

/** The feed width of a 640 px window: the album and the grid fit. */
export const УзкоеОкно: Story = {
  parameters: { narrow: true },
  render: () => (
    <>
      <Bubble attachments={manyPhotos(1)} text="Одно" />
      <Bubble attachments={manyPhotos(10)} text="Десять" />
    </>
  ),
};

/** English: the cell labels and the tiles are in English. */
export const Английский: Story = {
  globals: { language: "en" },
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("e1", "IMG_0001.jpg", 1280, 960, { broken: true })]} text="Balcony view" />
      <Bubble attachments={[...manyPhotos(3, "e"), demoPhoto("e9", "IMG_0009.jpg", 800, 600, { missing: true })]} text="Four photos, one missing" />
    </>
  ),
};

/** Large text: the grid keeps its shape, the tile text wraps. */
export const КрупныйТекст: Story = { globals: { textScale: "large" }, args: { images: manyPhotos(4) } };
