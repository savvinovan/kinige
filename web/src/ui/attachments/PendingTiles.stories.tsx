import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import type { AttachmentView } from "../../api/generated/AttachmentView";
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
  // The tiles stand in the composer above the field.
  render: (args) => <Composer items={args.items} />,
} satisfies Meta<typeof PendingTiles>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Two photos and a PDF, as above the field after choosing them in the dialog. The x removes a tile (it also works in the story). */
export const ДваФотоИPDF: Story = {};

/** Ten tiles in a 640 px window: the row wraps, every x is reachable. */
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

/** Large text: the file tiles grow with the text. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** The composer stand-in: the tiles above a quiet field; the x removes a tile. */
function Composer({ items }: { items: readonly AttachmentView[] }) {
  const [rest, setRest] = useState(items);
  return (
    <div className="attachments-composer">
      <PendingTiles items={rest} onRemove={(id) => setRest((was) => was.filter((item) => item.id !== id))} />
      <span className="attachments-composer__field">…</span>
    </div>
  );
}
