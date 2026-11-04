import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { staticVoice } from "../../voice/staticVoice";
import type { VoiceState } from "../../voice/useVoiceRecorder";
import { Composer } from "../composer/Composer";
import { Frame } from "../stories/Frame";

const noop = () => undefined;

// A fixed, plausible meter: no randomness, so the snapshots do not change from run to run.
const LEVELS = [0.1, 0.3, 0.2, 0.5, 0.7, 0.4, 0.3, 0.6, 0.9, 0.5, 0.2, 0.1, 0.3, 0.6, 0.8, 0.4, 0.3, 0.2, 0.5, 0.7, 0.3, 0.2, 0.1, 0.2];

function Demo({ state, text = "" }: { state: VoiceState; text?: string }) {
  const [value, setValue] = useState(text);
  return (
    <Frame bottom>
      <Composer value={value} onChange={setValue} onSubmit={noop} voice={staticVoice(state)} />
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

/** Recording at 0:12: the dot, the timer, the meter, the trash on the left and Send on the right. The bar keeps the height of the field. */
export const Recording: Story = { args: { state: { kind: "recording", elapsedMs: 12_000, levels: LEVELS } } };

/** From 29:00 the timer takes the warning colour. */
export const NearTheLimit: Story = { args: { state: { kind: "recording", elapsedMs: 29 * 60_000 + 20_000, levels: LEVELS } } };

/** With Reduce Motion there are no bars: only the dot and the timer. */
export const ReducedMotion: Story = {
  globals: { reduceMotion: "on" },
  args: { state: { kind: "recording", elapsedMs: 12_000, levels: LEVELS } },
};

/** A recording over five seconds asks before it is dropped; it goes on while it asks. */
export const Discard: Story = { args: { state: { kind: "confirming", elapsedMs: 12_000, levels: LEVELS } } };

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
export const English: Story = { globals: { language: "en" }, args: { state: { kind: "confirming", elapsedMs: 12_000, levels: LEVELS } } };

/** Russian, the discard question. */
export const Russian: Story = { globals: { language: "ru" }, args: { state: { kind: "notice", reason: "denied" } } };

/** Large text: the bar grows with the text. */
export const LargeText: Story = { globals: { textScale: "large" }, args: { state: { kind: "recording", elapsedMs: 12_000, levels: LEVELS } } };
