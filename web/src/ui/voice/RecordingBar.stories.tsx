import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { staticVoice } from "../../voice/staticVoice";
import type { VoiceState } from "../../voice/useVoiceRecorder";
import { Composer } from "../composer/Composer";
import { Frame } from "../stories/Frame";

const noop = () => undefined;

// A fixed, plausible run of levels (phrases and pauses): no randomness, so the snapshots do not change from run to run. The newest is last.
const SAMPLE = Array.from({ length: 240 }, (_, i) => {
  const phrase = 0.5 + 0.5 * Math.sin(i * 0.11 + 1);
  const syllable = Math.abs(Math.sin(i * 0.53) * Math.cos(i * 0.29 + 2));
  return Math.min(1, 0.04 + 0.5 * phrase * syllable);
});

/** The waveform is not moving here (`at: 0` is long ago, so the strip has finished sliding); the real one slides in from the right. */
const levels = (count: number) => ({ levels: SAMPLE.slice(-count), at: 0 });

function Demo({ state, text = "", narrow = false }: { state: VoiceState; text?: string; narrow?: boolean }) {
  const [value, setValue] = useState(text);
  return (
    <Frame bottom narrow={narrow}>
      <Composer value={value} onChange={setValue} onSubmit={noop} voice={staticVoice(state, { levels })} />
    </Frame>
  );
}

const meta = {
  title: "Components/Voice note: recording",
  component: Demo,
  args: { state: { kind: "idle" } },
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The microphone button on an empty field. */
export const Idle: Story = {};

/** Recording at 0:12: the trash on the left, the red dot and the timer, the live waveform across the whole middle, Send on the right. The bar keeps the height of the field. */
export const Recording: Story = { args: { state: { kind: "recording", elapsedMs: 12_000 } } };

/** The first moments: only a few bars have entered from the right, the rest of the field is empty. */
export const JustStarted: Story = {
  args: { state: { kind: "recording", elapsedMs: 1_000 } },
  render: ({ state }) => (
    <Frame bottom>
      <Composer value="" onChange={noop} onSubmit={noop} voice={staticVoice(state, { levels: (count) => ({ levels: SAMPLE.slice(0, 9).slice(-count), at: 0 }) })} />
    </Frame>
  ),
};

/** From 29:00 the timer takes the warning colour. */
export const NearTheLimit: Story = { args: { state: { kind: "recording", elapsedMs: 29 * 60_000 + 20_000 } } };

/** With Reduce Motion the strip does not slide, it steps ten times a second: the waveform is the level of the microphone, so it stays. */
export const ReducedMotion: Story = {
  globals: { reduceMotion: "on" },
  args: { state: { kind: "recording", elapsedMs: 12_000 } },
};

/** A recording over five seconds asks before it is dropped; it goes on while it asks. */
export const Discard: Story = { args: { state: { kind: "confirming", elapsedMs: 12_000 } } };

/** The headset was unplugged: nothing is lost, Send and Discard. */
export const DeviceLost: Story = { args: { state: { kind: "lost", elapsedMs: 12_000 } } };

/** A full disk: the bytes stay, Retry and Discard. */
export const SaveFailed: Story = { args: { state: { kind: "send-failed", elapsedMs: 40_000, error: "disk_full", retryable: true } } };

/** The chat is gone: nothing a second try could change, so Discard only, with the reason in words. */
export const SaveFailedForGood: Story = { args: { state: { kind: "send-failed", elapsedMs: 40_000, error: "chat_not_found", retryable: false } } };

/** Access is off in System Settings. */
export const AccessOff: Story = { args: { state: { kind: "notice", reason: "denied" } } };

export const NoMicrophone: Story = { args: { state: { kind: "notice", reason: "no-device" } } };

export const Busy: Story = { args: { state: { kind: "notice", reason: "busy" } } };

export const TooShort: Story = { args: { state: { kind: "notice", reason: "too-short" } } };

export const TooLong: Story = { args: { state: { kind: "notice", reason: "too-long" } } };

/** English. */
export const English: Story = { globals: { language: "en" }, args: { state: { kind: "recording", elapsedMs: 12_000 } } };

/** English, the discard question. */
export const EnglishDiscard: Story = { globals: { language: "en" }, args: { state: { kind: "confirming", elapsedMs: 12_000 } } };

/** Russian, access off. */
export const Russian: Story = { globals: { language: "ru" }, args: { state: { kind: "notice", reason: "denied" } } };

/** Large text: the bar grows with the text, and so do the bars of the waveform (they are in rem). */
export const LargeText: Story = { globals: { textScale: "large" }, args: { state: { kind: "recording", elapsedMs: 12_000 } } };

/** The 640 px window: the waveform takes what the timer and Send leave. */
export const NarrowWindow: Story = { args: { state: { kind: "recording", elapsedMs: 12_000 }, narrow: true } };
