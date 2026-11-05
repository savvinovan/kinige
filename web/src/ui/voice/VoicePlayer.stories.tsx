import type { Meta, StoryObj } from "@storybook/react-vite";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { Bubble, withScene } from "../attachments/AttachmentsStage";
import { demoPeaks, demoVoice } from "../attachments/demo";
import { MessageAttachments } from "../attachments/MessageAttachments";
import { VoicePlayer } from "./VoicePlayer";

const noop = () => undefined;

const NOTE = demoVoice("note", 40_000, { waveform: demoPeaks(1), byteSize: 21_196 });

/** A bubble whose player is shown in a given state; the media element is not involved. The time of the bubble sits on the meta line. */
function Playing({ attachment, playing, positionMs, text = "" }: { attachment: AttachmentView; playing: boolean; positionMs: number; text?: string }) {
  return (
    <Bubble
      attachments={[]}
      text={text}
      timeBeside={text === ""}
      slot={<VoicePlayer attachment={attachment} onOpenFile={noop} preview={{ playing, positionMs }} />}
    />
  );
}

const meta = {
  title: "Components/Voice note: player",
  component: VoicePlayer,
  args: { attachment: NOTE, onOpenFile: noop },
  decorators: [withScene],
  render: (args) => <Bubble attachments={[args.attachment]} timeBeside />,
} satisfies Meta<typeof VoicePlayer>;

export default meta;
type Story = StoryObj<typeof meta>;

/** At rest: the big round play button, the waveform (all muted), "0:40, 20.7 KB" under it and the bubble's time at the right. No audio has been requested. */
export const AtRest: Story = {};

/** Playing 12 s in: the pause button, the played part in accent (the fill stops inside a bar), the elapsed time and the speed chip. */
export const Playing12: Story = {
  render: () => <Playing attachment={NOTE} playing positionMs={12_300} />,
};

/** Paused part-way: the play button again, the fill stays where it stopped. */
export const Paused: Story = {
  render: () => <Playing attachment={NOTE} playing={false} positionMs={25_000} />,
};

/** The note has no peaks yet (an import, an older version, or the decoder could not read it): neutral bars of the same layout, so the bubble does not change size. */
export const PlaceholderBars: Story = {
  args: { attachment: demoVoice("no-peaks", 40_000, { byteSize: 21_196 }) },
};

/** The placeholder while playing: the fill still runs over it. */
export const PlaceholderPlaying: Story = {
  render: () => <Playing attachment={demoVoice("no-peaks", 40_000, { byteSize: 21_196 })} playing positionMs={18_000} />,
};

/** A very quiet note and a loud one in one bubble each: the loudest bar is always the full height. */
export const LoudAndQuiet: Story = {
  render: () => (
    <Bubble
      attachments={[]}
      slot={
        <MessageAttachments
          attachments={[
            demoVoice("quiet", 12_000, { waveform: demoPeaks(5).map((v) => Math.round(v * 0.25)), byteSize: 6_400 }),
            demoVoice("loud", 12_000, { waveform: demoPeaks(9), byteSize: 6_400 }),
          ]}
          onOpenImage={noop}
          onOpenFile={noop}
        />
      }
    />
  ),
};

/** An hour or more reads as h:mm:ss. */
export const LongNote: Story = {
  render: () => <Playing attachment={demoVoice("long", 3_723_000, { waveform: demoPeaks(3), byteSize: 29_784_000 })} playing positionMs={61_000} />,
};

/** A Telegram voice with no length in its row: the total is a dash until the file says. */
export const UnknownLength: Story = {
  args: { attachment: demoVoice("tg", null, { mime: "audio/ogg", filename: "audio_1.ogg", byteSize: 20_700 }) },
};

/** The file is gone: the slice 7 tile. */
export const FileMissing: Story = { args: { attachment: demoVoice("gone", 40_000, { state: "missing" }) } };

/** A file the web view will not play (here over 64 MiB, which the route refuses in every web view): "Can't play here" and "Open in default app". */
export const CantPlayHere: Story = {
  args: { attachment: demoVoice("big", 40_000, { byteSize: 70 * 1024 * 1024, filename: "interview.m4a" }) },
};

/** An edited voice note: the text sits under the player and the time goes after the text, as in any bubble. */
export const WithText: Story = {
  render: () => <Bubble attachments={[NOTE]} text="Idea for the weekend trip" />,
};

/** English, the same unplayable file. */
export const CantPlayHereEnglish: Story = {
  globals: { language: "en" },
  args: { attachment: demoVoice("big-en", 40_000, { byteSize: 70 * 1024 * 1024, filename: "interview.m4a" }) },
};

/** English. */
export const English: Story = { globals: { language: "en" }, render: () => <Playing attachment={NOTE} playing positionMs={12_300} /> };

/** Large text: the player grows with the text (the bars and the button are in rem) and the meta line stays on one line. */
export const LargeText: Story = {
  globals: { textScale: "large" },
  render: () => <Playing attachment={NOTE} playing positionMs={12_300} />,
};

/** The 640 px window. */
export const NarrowWindow: Story = { parameters: { narrow: true }, render: () => <Playing attachment={NOTE} playing positionMs={12_300} /> };

/** Several notes one under another, as in the feed: at rest, and one half played. */
export const SeveralNotes: Story = {
  render: () => (
    <>
      <Bubble attachments={[]} timeBeside slot={<MessageAttachments attachments={[demoVoice("n1", 5_000, { waveform: demoPeaks(2), byteSize: 20_700 })]} onOpenImage={noop} onOpenFile={noop} />} />
      <Playing attachment={demoVoice("n2", 2_000, { waveform: demoPeaks(4), byteSize: 11_300 })} playing={false} positionMs={900} />
      <Playing attachment={demoVoice("n3", 40_000, { waveform: demoPeaks(7), byteSize: 21_196 })} playing positionMs={31_000} />
    </>
  ),
};
