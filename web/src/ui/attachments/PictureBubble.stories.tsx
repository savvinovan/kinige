import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { ReplyQuote } from "../reply/ReplyQuote";
import { Bubble, manyPhotos, withScene } from "./AttachmentsStage";
import { demoFile, demoPhoto } from "./demo";

// KINIG-132: an entry that is only pictures fills its bubble edge to edge, as in Telegram, and the time is a pill on the picture.
// With text the picture is edge to edge at the top and the text sits under it with the usual padding.

const MEGABYTE = 1024 * 1024;
const CAPTION = "Скриншот поста, который хочется не потерять";
const LONG_CAPTION =
  "Скриншот поста, который хочется не потерять: в нём список из пяти пунктов, и каждый из них стоит перечитать ещё раз в конце недели";

const QUOTE: ReplyTo = {
  id: "old",
  chatId: "c",
  journalDate: "2023-10-31",
  excerpt: "Так будет лучше: перестать откладывать",
  state: "live",
  yearsLater: 3,
  attachments: { photos: 0, files: 0, voices: 0, voiceMs: null, name: null },
};
const quote = <ReplyQuote quote={QUOTE} onOpen={() => undefined} />;

const meta = {
  title: "Компоненты/Вложения: картинка без рамки",
  decorators: [withScene],
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** A landscape picture: no padding and no fill around it, the time in a pill at its bottom right. */
export const OnePicture: Story = {
  render: () => <Bubble attachments={[demoPhoto("one", "IMG_0001.jpg", 1280, 960)]} />,
};

/** A wide picture (16:9) and a tall one: the bubble takes the shape of the picture. */
export const WideAndTallPictures: Story = {
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("wide", "IMG_0002.jpg", 1600, 900)]} />
      <Bubble attachments={[demoPhoto("tall", "IMG_0003.jpg", 800, 1200)]} />
    </>
  ),
};

/** A grid of pictures: the cells fill the bubble, the bubble's corners clip the outer ones, the time is a pill on the last cell. */
export const PictureGrid: Story = {
  render: () => (
    <>
      <Bubble attachments={manyPhotos(2, "g2")} />
      <Bubble attachments={manyPhotos(4, "g4")} />
    </>
  ),
};

/** The pill reads on a near-white picture and on a near-black one. */
export const LightAndDarkPictures: Story = {
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("light", "IMG_0004.jpg", 1280, 800, { tone: "light" })]} />
      <Bubble attachments={[demoPhoto("dark", "IMG_0005.jpg", 1280, 800, { tone: "dark" })]} />
    </>
  ),
};

/** A star and the agent mark go into the pill with the time; nothing the footer showed is dropped. */
export const PillWithMarks: Story = {
  render: () => (
    <Bubble attachments={[demoPhoto("marks", "IMG_0006.jpg", 1280, 800)]} extra={{ favorite: true, byAgent: true }} />
  ),
};

/** A picture with text: edge to edge at the top, the text under it with the usual padding, the footer in its usual place. */
export const PictureWithText: Story = {
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("t1", "IMG_0007.jpg", 1280, 960)]} text={CAPTION} />
      <Bubble attachments={manyPhotos(3, "t3")} text={CAPTION} extra={{ favorite: true }} />
    </>
  ),
};

/** A long caption wraps at the picture's edge: it does not widen the bubble past the picture. */
export const PictureWithLongText: Story = {
  render: () => <Bubble attachments={[demoPhoto("long", "IMG_0008.jpg", 960, 960)]} text={LONG_CAPTION} />,
};

/** A reply quote above the pictures keeps its padding and sits on the bubble's fill; the picture starts under it and fills the bubble edge to edge. */
export const ReplyAbovePicture: Story = {
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("r1", "IMG_0009.jpg", 1280, 960)]} quote={quote} />
      <Bubble attachments={[demoPhoto("r2", "IMG_0010.jpg", 1280, 960)]} text={CAPTION} quote={quote} />
    </>
  ),
};

/** Pictures with a file: not only pictures, so the bubble is the plain one with its padding, as before. */
export const PictureAndFileKeepTheFrame: Story = {
  render: () => <Bubble attachments={[demoPhoto("m1", "IMG_0011.jpg", 1280, 960), demoFile("m2", "Меню.pdf", "file", 2.4 * MEGABYTE)]} />,
};

/** The action row beside the bubble is where it was; the pill is not in its way. */
export const ActionsBesidePicture: Story = {
  render: () => <Bubble attachments={[demoPhoto("a1", "IMG_0012.jpg", 1280, 960)]} extra={{ showActions: true }} />,
};

/** The jump target found by search: the picture hides the bubble's fill, so a ring shows it. */
export const Highlighted: Story = {
  render: () => <Bubble attachments={[demoPhoto("h1", "IMG_0013.jpg", 1280, 960)]} extra={{ highlighted: true }} />,
};

/** A picture whose file is missing, alone and in a grid with good pictures: the tile has the bubble's fill, so the entry still has a visible box. */
export const MissingPicture: Story = {
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("ms1", "IMG_0018.jpg", 1280, 960, { missing: true })]} />
      <Bubble attachments={[demoPhoto("ms2", "IMG_0019.jpg", 1280, 960), demoPhoto("ms3", "IMG_0020.jpg", 1280, 960, { missing: true }), demoPhoto("ms4", "IMG_0021.jpg", 960, 1280)]} />
    </>
  ),
};

/** A picture that cannot be decoded (no preview), alone and in a grid with good pictures. */
export const BrokenPicture: Story = {
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("br1", "IMG_0022.jpg", 1280, 960, { broken: true })]} />
      <Bubble attachments={[demoPhoto("br2", "IMG_0023.jpg", 1280, 960), demoPhoto("br3", "IMG_0024.jpg", 1280, 960, { broken: true }), demoPhoto("br4", "IMG_0025.jpg", 960, 1280)]} />
    </>
  ),
};

/** A very narrow picture (a 1170 by 12000 strip, 39 px wide): it keeps its size, centred on a strip of the bubble's fill, and the pill is whole. With text the caption has room. */
export const NarrowStrip: Story = {
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("ns1", "IMG_0026.jpg", 1170, 12000)]} />
      <Bubble attachments={[demoPhoto("ns2", "IMG_0027.jpg", 1170, 12000)]} text={CAPTION} />
    </>
  ),
};

/** The star, the agent mark and the time in the pill of a 100 px wide picture: the bubble is as wide as the pill needs. */
export const NarrowPictureWithMarks: Story = {
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("nm1", "IMG_0028.jpg", 100, 300)]} extra={{ favorite: true, byAgent: true }} />
      <Bubble attachments={[demoPhoto("nm2", "IMG_0029.jpg", 100, 300)]} extra={{ favorite: true }} />
    </>
  ),
};

/** English. */
export const English: Story = {
  globals: { language: "en" },
  render: () => <Bubble attachments={[demoPhoto("en", "IMG_0014.jpg", 1280, 960)]} />,
};

/** Large text: the pill and the caption grow with the text. */
export const LargeText: Story = {
  globals: { textScale: "large" },
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("l1", "IMG_0015.jpg", 1280, 960)]} />
      <Bubble attachments={[demoPhoto("l2", "IMG_0016.jpg", 1280, 960)]} text={CAPTION} />
    </>
  ),
};

/** A 640 px window: the picture shrinks with the column and stays edge to edge. */
export const NarrowWindow: Story = {
  parameters: { narrow: true },
  render: () => (
    <>
      <Bubble attachments={[demoPhoto("n1", "IMG_0017.jpg", 1600, 900)]} />
      <Bubble attachments={manyPhotos(4, "n4")} text={CAPTION} />
    </>
  ),
};
