import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ReactionView } from "../../api/generated/ReactionView";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import type { TrashItem } from "../../api/generated/TrashItem";
import { mockMessage } from "../../api/mockData";
import { Bubble, withScene } from "../attachments/AttachmentsStage";
import { DemoMedia, demoPhoto } from "../attachments/demo";
import { ReplyQuote } from "../reply/ReplyQuote";
import { Frame } from "../stories/Frame";
import { TrashView } from "../trash/TrashView";
import { StickerImage } from "./StickerImage";
import "./stickers-stories.css";

// A sticker in the feed (slice 5c), as the feed draws it: no bubble, the time in a pill on the picture, the chips under it, a quote above in a bubble of
// its own. An entry with text or a cover is an ordinary picture and is shown beside them for comparison. The pictures are drawn by code.
const CAT = "\u{1F63A}";
const sticker = (id: string, options: Parameters<typeof demoPhoto>[4] = {}, width = 512, height = 512) =>
  demoPhoto(id, "sticker.webp", width, height, { sticker: { emoji: CAT }, ...options });

const REACTIONS: ReactionView[] = [
  { emoji: "\u{2764}\u{FE0F}", putAt: Date.UTC(2026, 10, 20, 9, 30), putTz: "UTC", yearsLater: 0 },
  { emoji: "\u{1F602}", putAt: Date.UTC(2026, 10, 20, 9, 31), putTz: "UTC", yearsLater: 0 },
];

const QUOTE: ReplyTo = {
  id: "old",
  chatId: "c",
  journalDate: "2025-11-20",
  excerpt: "Так будет лучше: перестать откладывать",
  state: "live",
  yearsLater: 1,
  attachments: { photos: 0, files: 0, voices: 0, voiceMs: null, name: null, sticker: false, stickerEmoji: null },
};

function Sticker({ attachment, extra }: { attachment: ReturnType<typeof sticker>; extra?: Parameters<typeof Bubble>[0]["extra"] }) {
  return <Bubble attachments={[attachment]} slot={<StickerImage attachment={attachment} />} extra={{ sticker: true, ...extra }} />;
}

const meta = {
  title: "Компоненты/Стикер в ленте",
  decorators: [withScene],
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** Bare: no bubble, no frame; the long side is the token (160 px), the time is a pill on the corner. */
export const Голый: Story = { render: () => <Sticker attachment={sticker("b1")} /> };

/** A wide and a tall sticker: the long side is the same, the box follows the shape before the file loads. */
export const ШирокийИВысокий: Story = {
  render: () => (
    <>
      <Sticker attachment={sticker("w1", {}, 512, 256)} />
      <Sticker attachment={sticker("t1", {}, 256, 512)} />
    </>
  ),
};

/** Chips go under the sticker at its left edge; the time stays on the picture. */
export const СРеакциями: Story = { render: () => <Sticker attachment={sticker("c1")} extra={{ reactions: REACTIONS }} /> };

/** A star and the agent mark go into the pill with the time. */
export const СоЗвездой: Story = { render: () => <Sticker attachment={sticker("f1")} extra={{ favorite: true, byAgent: true }} /> };

/** A reply: the quote is a small bubble of its own above the sticker. */
export const Ответ: Story = {
  render: () => (
    <>
      <Sticker attachment={sticker("q1")} extra={{ quote: <ReplyQuote quote={QUOTE} onOpen={() => undefined} /> }} />
      <Sticker attachment={sticker("q2")} extra={{ quote: <ReplyQuote quote={QUOTE} onOpen={() => undefined} />, reactions: REACTIONS }} />
    </>
  ),
};

/** The hover row has no Edit: copy, task, favorite, more, and the reaction buttons. */
export const РядДействийБезПравки: Story = { render: () => <Sticker attachment={sticker("a1")} extra={{ showActions: true }} /> };

/** The jump target found by search: a ring along the picture. */
export const Выделенный: Story = { render: () => <Sticker attachment={sticker("h1")} extra={{ highlighted: true }} /> };

/** A sticker with no emoji, and one whose file is missing (the emoji stands in the box). */
export const БезЭмодзиИБезФайла: Story = {
  render: () => (
    <>
      <Sticker attachment={demoPhoto("n1", "sticker.webp", 512, 512, { sticker: { emoji: null } })} />
      <Sticker attachment={sticker("m1", { missing: true })} />
    </>
  ),
};

/** An entry edited after the import: it keeps its text and shows the sticker as a picture in the bubble, not as a sticker. */
export const ОтредактированнаяЗаписьИмпорта: Story = {
  render: () => <Bubble attachments={[sticker("e1")]} text="Это про котов, которых я так и не завёл" />,
};

/** A covered sticker is drawn as 7b's cover until it is revealed; it is not a sticker while covered. */
export const ЗакрытыйСтикер: Story = {
  render: () => <Bubble attachments={[sticker("s1", { spoiler: true })]} />,
};

/** In the narrow 640 px window. */
export const УзкоеОкно: Story = { render: () => <Sticker attachment={sticker("n2")} extra={{ reactions: REACTIONS }} />, parameters: { narrow: true } };

export const Английский: Story = {
  render: () => <Sticker attachment={sticker("en1")} extra={{ reactions: REACTIONS, showActions: true }} />,
  globals: { language: "en" },
};

export const КрупныйТекст: Story = {
  render: () => <Sticker attachment={sticker("lg1")} extra={{ reactions: REACTIONS, showActions: true }} />,
  globals: { textScale: "large" },
};

// ---- the trash ----

const trashItem = (id: string, bodyMd: string, attachment: ReturnType<typeof sticker>): TrashItem => ({
  message: mockMessage({ id, bodyMd, journalDate: "2026-11-19", attachments: [attachment], reactions: REACTIONS }),
  chatTitle: "Входящие",
  chatIcon: "📥",
  chatIsInbox: true,
  chatColor: "grey",
  deletedAt: Date.UTC(2026, 10, 19, 18, 0),
  daysLeft: 29,
});

/** In the trash a sticker is drawn as a sticker too, with its chips; restore and delete forever work as for any entry. */
export const ВКорзине: Story = {
  decorators: [],
  render: () => (
    <Frame surface="paper">
      <DemoMedia>
        <TrashView
          items={[trashItem("t-a", "", sticker("tr1")), trashItem("t-b", "Старая заметка", sticker("tr2")), trashItem("t-c", "", sticker("tr3", { spoiler: true }))]}
          language="ru"
          today="2026-11-20"
          onRestore={() => undefined}
          onPurge={() => undefined}
        />
      </DemoMedia>
    </Frame>
  ),
};
