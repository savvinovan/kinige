import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { staticVoice } from "../../voice/staticVoice";
import type { VoiceController, VoiceState } from "../../voice/useVoiceRecorder";
import { Composer } from "../composer/Composer";

function controls(state: VoiceState): VoiceController {
  return {
    ...staticVoice(state),
    start: vi.fn(),
    send: vi.fn(),
    requestCancel: vi.fn(),
    keepRecording: vi.fn(),
    discard: vi.fn(),
    retry: vi.fn(),
    openSettings: vi.fn(),
    dismissNotice: vi.fn(),
  };
}

function show(state: VoiceState, language: "en" | "ru" = "en") {
  const voice = controls(state);
  const result = render(<Composer value="" onChange={() => undefined} onSubmit={() => undefined} voice={voice} />, { language });
  return { voice, ...result };
}

describe("the microphone button", () => {
  it("is live with a recorder and starts on click; without one it is only drawn", () => {
    const live = show({ kind: "idle" });
    fireEvent.click(screen.getByRole("button", { name: "Record voice note" }));
    expect(live.voice.start).toHaveBeenCalledTimes(1);
    live.unmount();
    render(<Composer value="" onChange={() => undefined} />, { language: "en" });
    expect(screen.getByRole("button", { name: "Record voice note" })).toHaveAttribute("aria-disabled", "true");
  });

  it("is replaced by Send while there is text (PRD §5), so a note and text never mix", () => {
    render(<Composer value="text" onChange={() => undefined} onSubmit={() => undefined} voice={controls({ kind: "idle" })} />, { language: "en" });
    expect(screen.queryByRole("button", { name: "Record voice note" })).toBeNull();
    expect(screen.getByRole("button", { name: "Send" })).toBeInTheDocument();
  });
});

describe("the recording bar", () => {
  it("replaces the field with a dot, the timer, a level meter, the trash and Send; Send has the focus", () => {
    const { voice, container } = show({ kind: "recording", elapsedMs: 63_000, levels: [0, 0.5, 1] });
    expect(screen.getByRole("group", { name: "Recording, 1:03" })).toBeInTheDocument();
    expect(screen.getByText("1:03")).toHaveClass("recording-bar__timer");
    expect(container.querySelector(".recording-bar__dot")).not.toBeNull();
    expect(container.querySelectorAll(".recording-bar__bar")).toHaveLength(3);
    expect(container.querySelector(".composer__bar")).toHaveClass("composer__bar--hidden");
    const send = screen.getByRole("button", { name: "Send the voice note" });
    expect(send).toHaveFocus();
    fireEvent.click(send);
    expect(voice.send).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Discard the recording" }));
    expect(voice.requestCancel).toHaveBeenCalledTimes(1);
  });

  it("the timer turns to the warning colour from 29:00", () => {
    const before = show({ kind: "recording", elapsedMs: 28 * 60_000 + 59_000, levels: [] });
    expect(screen.getByText("28:59")).not.toHaveClass("recording-bar__timer--warn");
    before.unmount();
    show({ kind: "recording", elapsedMs: 29 * 60_000, levels: [] });
    expect(screen.getByText("29:00")).toHaveClass("recording-bar__timer--warn");
  });

  it("Escape cancels; while the question is open Escape keeps recording", () => {
    const recording = show({ kind: "recording", elapsedMs: 8000, levels: [] });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(recording.voice.requestCancel).toHaveBeenCalledTimes(1);
    recording.unmount();
    const asking = show({ kind: "confirming", elapsedMs: 8000, levels: [] });
    fireEvent.keyDown(document, { key: "Escape" });
    expect(asking.voice.keepRecording).toHaveBeenCalledTimes(1);
    expect(asking.voice.discard).not.toHaveBeenCalled();
  });

  it("asks 'Discard the recording?' with Discard and Keep recording, and the focus is on Keep", () => {
    const { voice } = show({ kind: "confirming", elapsedMs: 8000, levels: [] });
    expect(screen.getByRole("alertdialog", { name: "Discard the recording?" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Keep recording" })).toHaveFocus();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(voice.discard).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Keep recording" }));
    expect(voice.keepRecording).toHaveBeenCalledTimes(1);
  });

  it("a lost device says so with the time and no device name, and offers Send and Discard", () => {
    const { voice } = show({ kind: "lost", elapsedMs: 12_000 });
    expect(screen.getByText("Microphone disconnected · 0:12")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Send the voice note" }));
    expect(voice.send).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Discard the recording" }));
    expect(voice.discard).toHaveBeenCalled();
  });

  it("a failed save shows the error with Retry and Discard", () => {
    const { voice } = show({ kind: "send-failed", elapsedMs: 9000, error: "disk_full", retryable: true });
    expect(screen.getByRole("alert")).toHaveTextContent("The voice note was not saved. There is not enough disk space. Nothing was attached.");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(voice.retry).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(voice.discard).toHaveBeenCalled();
  });

  it("a save that cannot succeed on a second try offers Discard only, with the reason", () => {
    const { voice } = show({ kind: "send-failed", elapsedMs: 9000, error: "chat_not_found", retryable: false });
    expect(screen.getByRole("alert")).toHaveTextContent("The voice note was not saved. This chat was not found.");
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(voice.discard).toHaveBeenCalled();
  });

  it("speaks Russian", () => {
    show({ kind: "confirming", elapsedMs: 8000, levels: [] }, "ru");
    expect(screen.getByRole("button", { name: "Продолжить запись" })).toBeInTheDocument();
  });
});

describe("the line above the field", () => {
  it.each([
    ["denied", "Microphone access is off"],
    ["no-device", "No microphone found"],
    ["busy", "The microphone is unavailable"],
    ["too-short", "Too short to save"],
    ["too-long", "Too long to save"],
  ] as const)("%s", (reason, text) => {
    show({ kind: "notice", reason });
    expect(screen.getByRole("alert")).toHaveTextContent(text);
  });

  it("access off offers 'Open settings'; the others do not", () => {
    const denied = show({ kind: "notice", reason: "denied" });
    fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    expect(denied.voice.openSettings).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(denied.voice.dismissNotice).toHaveBeenCalledTimes(1);
    denied.unmount();
    show({ kind: "notice", reason: "no-device" });
    expect(screen.queryByRole("button", { name: "Open settings" })).toBeNull();
  });
});
