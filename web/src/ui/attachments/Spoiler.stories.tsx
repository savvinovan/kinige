import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useState } from "react";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { mockMessage } from "../../api/mockData";
import { staticVoice } from "../../voice/staticVoice";
import { Composer } from "../composer/Composer";
import { FeedRow } from "../feed/FeedRow";
import { Bubble, withScene } from "./AttachmentsStage";
import { demoFile, demoPhoto } from "./demo";
import { PendingTiles } from "./PendingTiles";
import { RevealProvider } from "./reveal";

// KINIG-136: a hidden photo is a flat cover in the photo's own box (no picture, no request) until its click; the description of a single photo is the
// entry's one line; a file card shows its new name and a description of up to two lines.

const MEGABYTE = 1024 * 1024;
const CAPTION = "Пляж, июль: закат над заливом";

const hidden = (id: string, name: string, width: number, height: number, extra: { description?: string; missing?: boolean } = {}) =>
  demoPhoto(id, name, width, height, { spoiler: true, ...extra });

const meta = {
  title: "Компоненты/Вложения: спойлер",
  decorators: [withScene],
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** A hidden photo alone in a frameless bubble: the cover fills it edge to edge and the time pill stays on the cover. */
export const CoveredPictureWithPill: Story = {
  render: () => (
    <RevealProvider viewKey="s">
      <Bubble attachments={[hidden("c1", "IMG_0001.jpg", 1280, 960)]} />
    </RevealProvider>
  ),
};

/** A hidden photo with a description and no text: the muted "Description hidden" is the entry's line and the time sits at its end. */
export const CoveredWithDescriptionHidden: Story = {
  render: () => (
    <RevealProvider viewKey="s">
      <Bubble
        attachments={[hidden("c2", "IMG_0002.jpg", 1280, 960, { description: CAPTION })]}
        extra={{ caption: { text: "Описание скрыто", muted: true } }}
      />
    </RevealProvider>
  ),
};

/** The same photo revealed: its description replaces the muted line in the same place, the bubble does not change its height; "Hide" shows on hover. */
export const RevealedWithCaption: Story = {
  render: () => (
    <RevealProvider viewKey="s" initial={["c3"]}>
      <Bubble
        attachments={[hidden("c3", "IMG_0003.jpg", 1280, 960, { description: CAPTION })]}
        extra={{ caption: { text: CAPTION, muted: false } }}
      />
    </RevealProvider>
  ),
};

/** A long description is cut to one line with an ellipsis; the time stays at its end. */
export const LongCaptionIsCut: Story = {
  render: () => (
    <RevealProvider viewKey="s">
      <Bubble
        attachments={[demoPhoto("c4", "IMG_0004.jpg", 1280, 960, { description: CAPTION })]}
        extra={{ caption: { text: `${CAPTION}, ${CAPTION}, ${CAPTION}, ${CAPTION}`, muted: false } }}
      />
    </RevealProvider>
  ),
};

/** An album of four with one covered cell (a word) and the others as they are; a row of three shows the eye only. */
export const AlbumWithACoveredCell: Story = {
  render: () => (
    <RevealProvider viewKey="s">
      <Bubble
        attachments={[
          demoPhoto("a1", "IMG_0001.jpg", 1280, 960),
          hidden("a2", "IMG_0002.jpg", 960, 1280),
          demoPhoto("a3", "IMG_0003.jpg", 1600, 900),
          demoPhoto("a4", "IMG_0004.jpg", 1024, 1024),
        ]}
      />
      <Bubble
        attachments={[
          hidden("b1", "IMG_0001.jpg", 1280, 960),
          hidden("b2", "IMG_0002.jpg", 960, 1280),
          hidden("b3", "IMG_0003.jpg", 1600, 900),
          demoPhoto("b4", "IMG_0004.jpg", 1024, 1024),
          demoPhoto("b5", "IMG_0005.jpg", 800, 1200),
          demoPhoto("b6", "IMG_0006.jpg", 1280, 960),
        ]}
      />
    </RevealProvider>
  ),
};

/** A tall screenshot strip, covered, with a caption (so the bubble keeps its floor width): the cover keeps its own narrow size centred on the strip of the bubble's fill. */
export const CoveredTallStrip: Story = {
  render: () => (
    <RevealProvider viewKey="s">
      <Bubble attachments={[hidden("t1", "Screenshot.png", 390, 2400)]} text="Скриншот" />
    </RevealProvider>
  ),
};

/** A hidden photo whose file is missing shows the cover, never "File is missing" with a name. */
export const CoveredMissingFile: Story = {
  render: () => (
    <RevealProvider viewKey="s">
      <Bubble attachments={[hidden("m1", "IMG_0001.jpg", 1280, 960, { missing: true })]} />
    </RevealProvider>
  ),
};

/** A hidden photo with text: the cover is edge to edge at the top and the text sits under it. */
export const CoveredWithText: Story = {
  render: () => (
    <RevealProvider viewKey="s">
      <Bubble attachments={[hidden("w1", "IMG_0001.jpg", 1280, 960)]} text="Скриншот, который не хочется показывать всем" />
    </RevealProvider>
  ),
};

/** A file card with its display name and a description of two lines (more is cut). */
export const FileCardWithDescription: Story = {
  render: () => (
    <Bubble
      attachments={[
        demoFile("f1", "scan0042.pdf", "file", 2.4 * MEGABYTE, "local", {
          name: "Договор аренды.pdf",
          description: "Подписанный экземпляр, страницы 1-6. Второй экземпляр лежит у арендодателя, оригинал заберу в пятницу после работы.",
        }),
      ]}
    />
  ),
};

/** Narrow window (640 px). */
export const NarrowWindow: Story = {
  parameters: { narrow: true },
  render: () => (
    <RevealProvider viewKey="s">
      <Bubble attachments={[hidden("n1", "IMG_0001.jpg", 1280, 960, { description: CAPTION })]} extra={{ caption: { text: "Описание скрыто", muted: true } }} />
    </RevealProvider>
  ),
};

/** English. */
export const English: Story = {
  globals: { language: "en" },
  render: () => (
    <RevealProvider viewKey="s">
      <Bubble attachments={[hidden("e1", "IMG_0001.jpg", 1280, 960, { description: "Beach, July" })]} extra={{ caption: { text: "Description hidden", muted: true } }} />
    </RevealProvider>
  ),
};

/** Large text: the cover's word and the caption grow with the text; the cover's box is content and keeps its size. */
export const LargeText: Story = {
  globals: { textScale: "large" },
  render: () => (
    <RevealProvider viewKey="s">
      <Bubble attachments={[hidden("l1", "IMG_0001.jpg", 1280, 960, { description: CAPTION })]} extra={{ caption: { text: "Описание скрыто", muted: true } }} />
    </RevealProvider>
  ),
};

/** A real feed row (the caption and the cover come from the same code as in the window): pressing the cover must not move or resize the bubble. */
function RowsInFeed() {
  const when = Date.UTC(2026, 9, 31, 9, 12);
  const row = (id: string, photo: AttachmentView) => (
    <FeedRow
      key={id}
      message={mockMessage({ id, bodyMd: "", createdAt: when, attachments: [photo] })}
      inSeries={false}
      marked={false}
      mode="view"
      onOpenImage={() => undefined}
    />
  );
  return (
    <RevealProvider viewKey="feed">
      <ol className="feed">
        {row("row-described", hidden("r1", "IMG_0001.jpg", 1280, 960, { description: CAPTION }))}
        {row("row-plain", hidden("r2", "IMG_0002.jpg", 1280, 960))}
      </ol>
    </RevealProvider>
  );
}

/** Two rows of the feed, a hidden photo with a description and one without: a click on a cover shows that photo and changes no size (a Playwright test measures it). */
export const RevealInTheFeed: Story = { render: () => <RowsInFeed /> };

const TILES: AttachmentView[] = [
  hidden("p1", "IMG_0001.jpg", 1280, 960),
  demoPhoto("p2", "IMG_0002.jpg", 960, 1280, { spoiler: true, name: "Закат.jpg" }),
  demoPhoto("p3", "IMG_0003.jpg", 1280, 960),
  demoFile("p4", "scan0042.pdf", "file", 2.4 * MEGABYTE, "local", { name: "Договор.pdf", description: "Подписанный" }),
];

function Tiles({ revealed = [], focus }: { revealed?: string[]; focus?: string }) {
  const [value, setValue] = useState("");
  useEffect(() => {
    if (focus) document.querySelector<HTMLElement>(focus)?.focus();
  }, [focus]);
  return (
    <RevealProvider viewKey="tiles" initial={revealed}>
      <Composer
        value={value}
        onChange={setValue}
        onSubmit={() => setValue("")}
        onAttach={() => undefined}
        voice={staticVoice({ kind: "idle" })}
        tiles={<PendingTiles items={TILES} onRemove={() => undefined} onOpen={() => undefined} onDetails={() => undefined} />}
        hasTiles
      />
    </RevealProvider>
  );
}

/** The composer's tiles: a covered photo with the "Details" pencil and the x on top, a revealed one, a plain one and a file with its new name. */
export const PendingTilesWithACover: Story = { render: () => <Tiles /> };

/** The first click reveals the covered tile in place; this is the state after it. */
export const PendingTileRevealed: Story = { render: () => <Tiles revealed={["p1"]} /> };

/** The keyboard focus on "Details" of a covered tile: the ring is not clipped. */
export const PendingTileDetailsFocus: Story = { render: () => <Tiles focus='[data-details-for="p1"]' /> };
