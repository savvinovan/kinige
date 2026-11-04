import type { Meta, StoryObj } from "@storybook/react-vite";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { Bubble, withScene } from "../attachments/AttachmentsStage";
import { demoVoice } from "../attachments/demo";
import { MessageAttachments } from "../attachments/MessageAttachments";
import { VoicePlayer } from "./VoicePlayer";

const noop = () => undefined;

const NOTE = demoVoice("note", 40_000);

/** A bubble whose player is shown in a given state; the media element is not involved. */
function Playing({ attachment, playing, positionMs, text = "" }: { attachment: AttachmentView; playing: boolean; positionMs: number; text?: string }) {
  return (
    <Bubble attachments={[]} text={text} slot={<VoicePlayer attachment={attachment} onOpenFile={noop} preview={{ playing, positionMs }} />} />
  );
}

const meta = {
  title: "Components/Voice note: player",
  component: VoicePlayer,
  args: { attachment: NOTE, onOpenFile: noop },
  decorators: [withScene],
  render: (args) => <Bubble attachments={[args.attachment]} />,
} satisfies Meta<typeof VoicePlayer>;

export default meta;
type Story = StoryObj<typeof meta>;

/** At rest: play, the seek bar, the total "0:40" and "1×". No audio has been requested. */
export const AtRest: Story = {};

/** Playing: "0:12 / 0:40", the pause button, the filled part of the seek bar. */
export const Playing12: Story = {
  render: () => <Playing attachment={NOTE} playing positionMs={12_000} />,
};

/** Paused part-way. */
export const Paused: Story = {
  render: () => <Playing attachment={NOTE} playing={false} positionMs={25_000} />,
};

/** An hour or more reads as h:mm:ss. */
export const LongNote: Story = {
  render: () => <Playing attachment={demoVoice("long", 3_723_000)} playing positionMs={61_000} />,
};

/** A Telegram voice with no length in its row: the total is a dash until the file says. */
export const UnknownLength: Story = { args: { attachment: demoVoice("tg", null, { mime: "audio/ogg", filename: "audio_1.ogg" }) } };

/** The file is gone: the slice 7 tile. */
export const FileMissing: Story = { args: { attachment: demoVoice("gone", 40_000, { state: "missing" }) } };

/** A file the web view will not play (here over 64 MiB, which the route refuses in every web view): "Can't play here" and "Open in default app". */
export const CantPlayHere: Story = {
  args: { attachment: demoVoice("big", 40_000, { byteSize: 70 * 1024 * 1024, filename: "interview.m4a" }) },
};

/** An edited voice note: the text sits under the player. */
export const WithText: Story = {
  render: () => <Bubble attachments={[NOTE]} text="Idea for the weekend trip" />,
};

/** English, the same unplayable file. */
export const CantPlayHereEnglish: Story = {
  globals: { language: "en" },
  args: { attachment: demoVoice("big-en", 40_000, { byteSize: 70 * 1024 * 1024, filename: "interview.m4a" }) },
};

/** English. */
export const English: Story = { globals: { language: "en" }, render: () => <Playing attachment={NOTE} playing positionMs={12_000} /> };

/** Large text: the player keeps its fixed width and the time stays on one line. */
export const LargeText: Story = {
  globals: { textScale: "large" },
  render: () => <Playing attachment={NOTE} playing positionMs={12_000} />,
};

/** The 640 px window. */
export const NarrowWindow: Story = { parameters: { narrow: true }, render: () => <Playing attachment={NOTE} playing positionMs={12_000} /> };

/** Two notes in one entry. */
export const TwoNotes: Story = {
  render: () => (
    <Bubble attachments={[]} slot={<MessageAttachments attachments={[NOTE, demoVoice("n2", 7_000)]} onOpenImage={noop} onOpenFile={noop} />} />
  ),
};
