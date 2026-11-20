import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { captureVoiceApi } from "../capture/captureVoiceApi";
import { INBOX_ID } from "../api/KinigeApi";
import { MockApi } from "../api/MockApi";
import { type FakeMedia, installFakeMedia, uninstallFakeMedia } from "../test/fakeMedia";
import { act, renderHook } from "../test/render";
import { type VoiceControls, useVoiceRecorder } from "./useVoiceRecorder";

// Two windows record (KINIG-153): the main window and the quick capture window, each with its own recorder and one microphone between them.
// The claim is the mock's copy of Rust's `MicrophoneClaim` (the Rust side has its own tests in `src-tauri/tests/ipc_capture_voice.rs`).

let media: FakeMedia;

beforeEach(() => {
  vi.useFakeTimers();
  media = installFakeMedia();
});

afterEach(() => {
  vi.useRealTimers();
  uninstallFakeMedia();
});

/** Both windows on one API, as the two pages of the app share one Rust side. */
function windows() {
  const api = new MockApi();
  const capture = captureVoiceApi(api);
  const onSentMain = vi.fn();
  const onSentCapture = vi.fn();
  const main = renderHook(() => useVoiceRecorder({ api, chatId: INBOX_ID, replyToId: null, onSent: onSentMain }));
  const quick = renderHook(() => useVoiceRecorder({ api: capture, chatId: INBOX_ID, replyToId: null, onSent: onSentCapture }));
  return { api, main, quick, onSentMain, onSentCapture };
}

async function begin(controls: { current: VoiceControls }) {
  await act(async () => {
    controls.current.start();
    await vi.advanceTimersByTimeAsync(10);
  });
}

async function settle(action: () => unknown) {
  await act(async () => {
    action();
    await vi.advanceTimersByTimeAsync(10);
  });
}

async function record(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

describe("one microphone, one recording", () => {
  it("is claimed before the device is opened and let go when the recording ends", async () => {
    const { api, main } = windows();
    expect(api.microphoneHolder).toBeNull();
    await begin(main.result);
    expect(main.result.current.state.kind).toBe("recording");
    expect(api.microphoneHolder).toBe("main");
    await settle(() => main.result.current.discard());
    expect(api.microphoneHolder).toBeNull();
    expect(media.streams[0]?.track.stopped).toBe(true);
  });

  it("the capture window is refused while the main window records, and the device is opened once", async () => {
    const { api, main, quick } = windows();
    await begin(main.result);
    await begin(quick.result);
    expect(quick.result.current.state).toEqual({ kind: "notice", reason: "busy" });
    expect(media.getUserMedia).toHaveBeenCalledTimes(1);
    expect(api.microphoneHolder).toBe("main");
    // The refusal took nothing and gave nothing back: the main window still has its claim after the other window dismisses the line.
    act(() => quick.result.current.dismissNotice());
    expect(api.microphoneHolder).toBe("main");
    // Once the main window is done, the capture window may record.
    await settle(() => main.result.current.discard());
    await begin(quick.result);
    expect(quick.result.current.state.kind).toBe("recording");
    expect(api.microphoneHolder).toBe("capture");
    expect(media.getUserMedia).toHaveBeenCalledTimes(2);
  });

  it("the main window is refused while the capture window records", async () => {
    const { api, main, quick } = windows();
    await begin(quick.result);
    await begin(main.result);
    expect(main.result.current.state).toEqual({ kind: "notice", reason: "busy" });
    expect(media.getUserMedia).toHaveBeenCalledTimes(1);
    expect(api.microphoneHolder).toBe("capture");
  });

  it("a claim that cannot be answered is no microphone: nothing is opened", async () => {
    const { api, quick } = windows();
    api.failNext("captureHoldMicrophone", "internal");
    await begin(quick.result);
    expect(quick.result.current.state).toEqual({ kind: "notice", reason: "busy" });
    expect(media.getUserMedia).not.toHaveBeenCalled();
    expect(api.microphoneHolder).toBeNull();
  });

  it.each([
    ["Send", (controls: VoiceControls) => controls.send(), 3000],
    ["a short cancel", (controls: VoiceControls) => controls.requestCancel(), 1000],
    ["Discard", (controls: VoiceControls) => controls.discard(), 3000],
    ["a lost device", () => media.streams[0]?.track.dispatchEvent(new Event("ended")), 3000],
  ])("%s lets the microphone go", async (_name, end, ms) => {
    const { api, quick } = windows();
    await begin(quick.result);
    await record(ms);
    expect(api.microphoneHolder).toBe("capture");
    await settle(() => end(quick.result.current));
    expect(api.microphoneHolder).toBeNull();
    expect(media.streams[0]?.track.stopped).toBe(true);
  });

  it("a refused permission lets it go too", async () => {
    const { api, quick } = windows();
    media.refuse("NotAllowedError");
    await begin(quick.result);
    expect(quick.result.current.state).toEqual({ kind: "notice", reason: "denied" });
    expect(api.microphoneHolder).toBeNull();
  });

  it("closing the window mid-recording lets it go", async () => {
    const { api, main, quick } = windows();
    await begin(quick.result);
    quick.unmount();
    await record(10);
    expect(api.microphoneHolder).toBeNull();
    expect(media.streams[0]?.track.stopped).toBe(true);
    await begin(main.result);
    expect(main.result.current.state.kind).toBe("recording");
  });
});

describe("a window that goes away while recording (the capture window hides)", () => {
  it("stops the recording, keeps it for Send, and releases the microphone", async () => {
    const { api, main, quick, onSentCapture } = windows();
    await begin(quick.result);
    await record(12_000);
    await settle(() => quick.result.current.park());
    expect(quick.result.current.state).toMatchObject({ kind: "lost", cause: "hidden" });
    expect(media.streams[0]?.track.stopped).toBe(true);
    expect(api.microphoneHolder).toBeNull();
    expect(api.sentVoices).toHaveLength(0);
    // Nothing is lost: when the window comes back, Send saves what was recorded, to the Inbox, and the other window is free to record meanwhile.
    await begin(main.result);
    expect(main.result.current.state.kind).toBe("recording");
    await settle(() => main.result.current.discard());
    await settle(() => quick.result.current.send());
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]?.chatId).toBe(INBOX_ID);
    expect(api.sentVoices[0]?.durationMs).toBeGreaterThanOrEqual(12_000);
    expect(onSentCapture).toHaveBeenCalledTimes(1);
    expect(quick.result.current.state.kind).toBe("idle");
  });

  it("the question 'Discard the recording?' is closed by it, and the recording is kept all the same", async () => {
    const { quick } = windows();
    await begin(quick.result);
    await record(9000);
    act(() => quick.result.current.requestCancel());
    expect(quick.result.current.state.kind).toBe("confirming");
    await settle(() => quick.result.current.park());
    expect(quick.result.current.state).toMatchObject({ kind: "lost", cause: "hidden" });
  });

  it("and Discard drops what was kept", async () => {
    const { api, quick } = windows();
    await begin(quick.result);
    await record(5000);
    await settle(() => quick.result.current.park());
    await settle(() => quick.result.current.discard());
    expect(quick.result.current.state.kind).toBe("idle");
    expect(api.sentVoices).toHaveLength(0);
  });

  it("under a second there is nothing worth keeping: the 'too short' line", async () => {
    const { api, quick } = windows();
    await begin(quick.result);
    await record(300);
    await settle(() => quick.result.current.park());
    expect(quick.result.current.state).toEqual({ kind: "notice", reason: "too-short" });
    expect(api.microphoneHolder).toBeNull();
    expect(media.streams[0]?.track.stopped).toBe(true);
  });

  it("while the system is still asking, the recorder is dropped the moment it starts", async () => {
    const { api, quick } = windows();
    const prompt = media.hold();
    await begin(quick.result);
    expect(quick.result.current.state.kind).toBe("starting");
    expect(api.microphoneHolder).toBe("capture");
    await settle(() => quick.result.current.park());
    expect(quick.result.current.state.kind).toBe("starting");
    await settle(() => prompt.answer());
    expect(quick.result.current.state.kind).toBe("idle");
    expect(media.streams[0]?.track.stopped).toBe(true);
    expect(api.microphoneHolder).toBeNull();
    expect(api.sentVoices).toHaveLength(0);
  });

  it("does nothing when nothing is recording", async () => {
    const { api, quick } = windows();
    await settle(() => quick.result.current.park());
    expect(quick.result.current.state.kind).toBe("idle");
    expect(api.microphoneHolder).toBeNull();
    // ...and does not leave a flag behind that would drop the next recording.
    await begin(quick.result);
    expect(quick.result.current.state.kind).toBe("recording");
  });
});

describe("the capture window's API", () => {
  it("sends every note to the Inbox, whatever chat or reply the recorder names", async () => {
    const api = new MockApi();
    const sent = vi.spyOn(api, "captureSendVoice");
    const adapter = captureVoiceApi(api);
    const note = await adapter.sendVoice("some-other-chat", new Uint8Array([1, 2, 3]), 5000, "some-reply", new Uint8Array(64));
    expect(sent).toHaveBeenCalledWith(new Uint8Array([1, 2, 3]), 5000, new Uint8Array(64));
    expect(note.chatId).toBe(INBOX_ID);
    expect(note.replyTo).toBeNull();
  });
});
