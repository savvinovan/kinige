import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { Composer } from "../composer/Composer";
import { manyPhotos, withScene } from "./AttachmentsStage";
import { demoFile, demoPhoto } from "./demo";
import { PendingTiles } from "./PendingTiles";

const MEGABYTE = 1024 * 1024;

const MIXED: AttachmentView[] = [
  demoPhoto("t1", "IMG_0001.jpg", 1280, 960),
  demoPhoto("t2", "IMG_0002.jpg", 960, 1280),
  demoFile("tpdf", "Меню.pdf", "file", 2.4 * MEGABYTE),
];

const meta = {
  title: "Компоненты/Вложения: плитки над полем",
  component: PendingTiles,
  args: { items: MIXED, onRemove: () => undefined },
  decorators: [withScene],
  // The tiles stand inside the composer's box, above the text line.
  render: (args) => <ComposerWithTiles items={args.items} />,
} satisfies Meta<typeof PendingTiles>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Two photos and a PDF, as in the box after choosing them in the dialog. The x removes a tile, a click on a photo would open the viewer. */
export const ДваФотоИPDF: Story = {};

/** One photo: a tile of 144 px height with the photo's own shape. */
export const ОдноФото: Story = { args: { items: [demoPhoto("one", "IMG_0001.jpg", 1280, 960)] } };

/** A dark photo on the dark box: the surface and the hairline keep the tile visible. */
export const ТёмноеФото: Story = { args: { items: [demoPhoto("dark", "night.jpg", 1280, 960, { tone: "dark" })] } };

/** A light photo: the x has its own fill and stays visible over it. */
export const СветлоеФото: Story = { args: { items: [demoPhoto("light", "snow.jpg", 1280, 960, { tone: "light" })] } };

/** A very wide photo (past the width cap: shown whole on the tile surface, never cropped), a wide one, a square one and a tall one. */
export const ШирокоеИВысокое: Story = {
  args: {
    items: [
      demoPhoto("w1", "panorama.jpg", 3000, 1000),
      demoPhoto("w2", "IMG_0002.jpg", 1600, 900),
      demoPhoto("w3", "IMG_0003.jpg", 1024, 1024),
      demoPhoto("w4", "IMG_0004.jpg", 800, 2400),
    ],
  },
};

/** A file that is not a photo: the icon of the feed, the name and the size. */
export const ОдинФайл: Story = { args: { items: [demoFile("one-pdf", "Договор аренды.pdf", "file", 2.4 * MEGABYTE)] } };

/**
 * The most the composer takes (ten) in a 640 px window: the tiles wrap, no horizontal scroll bar; past two lines (or 36% of the window
 * height) the area scrolls inside, and a half-shown line says there is more.
 */
export const Десять: Story = {
  parameters: { narrow: true },
  args: {
    items: [
      ...manyPhotos(7, "q"),
      demoFile("q-pdf", "Договор аренды 2026.pdf", "file", 2.4 * MEGABYTE),
      demoFile("q-mov", "Прогулка у реки.mp4", "video", 148 * MEGABYTE),
      demoFile("q-mp3", "Голосовая заметка.mp3", "audio", 3.1 * MEGABYTE),
    ],
  },
};

/** A very long file name is cut in the middle in a tile; an undecodable photo and a missing one keep the tile's size. */
export const ДлинноеИмяИОшибки: Story = {
  args: {
    items: [
      demoFile("lg", "Квартальный отчёт по продажам и расходам, финальная версия (2026).xlsx", "file", 18 * MEGABYTE),
      demoPhoto("br", "IMG_0100.jpg", 1280, 960, { broken: true }),
      demoPhoto("ms", "IMG_0101.jpg", 1280, 960, { missing: true }),
      demoFile("ms-file", "Договор.pdf", "file", 2.4 * MEGABYTE, "missing"),
    ],
  },
};

/** English. */
export const Английский: Story = {
  globals: { language: "en" },
  args: { items: [demoPhoto("e1", "IMG_0001.jpg", 1280, 960), demoFile("e2", "menu.pdf", "file", 2.4 * MEGABYTE)] },
};

/** Large text: the file tiles and the text line grow with the text; the photo tiles are content and keep their size. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** The real composer with the tiles inside its box; the x removes a tile. */
function ComposerWithTiles({ items }: { items: readonly AttachmentView[] }) {
  const [rest, setRest] = useState(items);
  const [value, setValue] = useState("");
  return (
    <Composer
      value={value}
      onChange={setValue}
      onSubmit={() => setValue("")}
      onAttach={() => undefined}
      tiles={<PendingTiles items={rest} onRemove={(id) => setRest((was) => was.filter((item) => item.id !== id))} onOpen={() => undefined} />}
      hasTiles={rest.length > 0}
    />
  );
}
