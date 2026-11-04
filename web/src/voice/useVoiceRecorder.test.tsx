import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../api/KinigeApi";
import { MockApi } from "../api/MockApi";
import { act, renderHook } from "../test/render";
import { CONFIRM_AFTER_MS, MAX_RECORDING_BYTES, MAX_RECORDING_MS } from "./recorderSession";
import { type VoiceControls, useVoiceRecorder } from "./useVoiceRecorder";

// A fake `MediaDevices` and `MediaRecorder`: the real pipeline of the hook runs with no device and no TCC.

const LABEL = "Alex's AirPods Pro";
const OTHER_CHAT = "chat-2";

class FakeTrack extends EventTarget {
  label = LABEL;
  stopped = false;
  stop() {
    this.stopped = true;
  }
}

class FakeStream {
  readonly track = new FakeTrack();
  getAudioTracks() {
    return [this.track];
  }
  getTracks() {
    return [this.track];
  }
}

class FakeRecorder extends EventTarget {
  static last: FakeRecorder | null = null;
  state: "inactive" | "recording" = "inactive";
  readonly options: unknown;
  constructor(_stream: unknown, options: unknown) {
    super();
    this.options = options;
    FakeRecorder.last = this;
  }
  static isTypeSupported(type: string) {
    return type === "audio/mp4;codecs=mp4a.40.2";
  }
  start() {
    this.state = "recording";
  }
  /** The engine delivers a chunk of `size` bytes. */
  chunk(size: number) {
    const data = new Blob([new Uint8Array(size).fill(7)]);
    const event = new Event("dataavailable") as Event & { data: Blob };
    event.data = data;
    this.dispatchEvent(event);
  }
  stop() {
    this.state = "inactive";
    this.chunk(16);
    this.dispatchEvent(new Event("stop"));
  }
}

let stream: FakeStream;
let getUserMedia: ReturnType<typeof vi.fn>;

function allow() {
  getUserMedia.mockImplementation(() => {
    stream = new FakeStream();
    return Promise.resolve(stream);
  });
}

function refuse(name: string) {
  getUserMedia.mockImplementation(() => Promise.reject(new DOMException("refused", name)));
}

beforeEach(() => {
  vi.useFakeTimers();
  getUserMedia = vi.fn();
  Object.defineProperty(navigator, "mediaDevices", { value: { getUserMedia }, configurable: true });
  vi.stubGlobal("MediaRecorder", FakeRecorder);
  FakeRecorder.last = null;
  allow();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(navigator, "mediaDevices");
});

function setup(options: { chatId?: string | null; replyToId?: string | null } = {}) {
  const api = new MockApi({ chats: [{ id: OTHER_CHAT, title: "Books", icon: "📚" }] });
  const onSent = vi.fn();
  const props = { chatId: options.chatId === undefined ? INBOX_ID : options.chatId, replyToId: options.replyToId ?? null };
  const hook = renderHook((current: typeof props) => useVoiceRecorder({ api, ...current, onSent }), { initialProps: props });
  return { api, onSent, hook, props };
}

async function begin(controls: { current: VoiceControls }) {
  await act(async () => {
    controls.current.start();
    await Promise.resolve();
  });
}

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

async function finish(controls: { current: VoiceControls }) {
  await act(async () => {
    controls.current.send();
    await vi.advanceTimersByTimeAsync(10);
  });
}

describe("the microphone is asked for only on the click", () => {
  it("does nothing at startup and starts on the first click", async () => {
    const { hook } = setup();
    expect(getUserMedia).not.toHaveBeenCalled();
    expect(hook.result.current.state.kind).toBe("idle");
    await begin(hook.result);
    expect(getUserMedia).toHaveBeenCalledTimes(1);
    expect(getUserMedia).toHaveBeenCalledWith({ audio: true });
    expect(hook.result.current.state.kind).toBe("recording");
    expect(FakeRecorder.last?.options).toMatchObject({ audioBitsPerSecond: 64_000, mimeType: "audio/mp4;codecs=mp4a.40.2" });
  });

  it("nothing starts in the trash, and a second click while recording changes nothing", async () => {
    const trash = setup({ chatId: null });
    await begin(trash.hook.result);
    expect(getUserMedia).not.toHaveBeenCalled();
    const open = setup();
    await begin(open.hook.result);
    await begin(open.hook.result);
    expect(getUserMedia).toHaveBeenCalledTimes(1);
  });
});

describe("errors, with no dialog", () => {
  it.each([
    ["NotAllowedError", "denied"],
    ["SecurityError", "denied"],
    ["NotFoundError", "no-device"],
    ["OverconstrainedError", "no-device"],
    ["NotReadableError", "busy"],
    ["AbortError", "busy"],
    ["SomethingNew", "busy"],
  ])("%s becomes %s", async (name, reason) => {
    refuse(name);
    const { hook } = setup();
    await begin(hook.result);
    expect(hook.result.current.state).toEqual({ kind: "notice", reason });
    act(() => hook.result.current.dismissNotice());
    expect(hook.result.current.state.kind).toBe("idle");
  });

  it("a web view without mediaDevices or MediaRecorder is the same as no microphone", async () => {
    Reflect.deleteProperty(navigator, "mediaDevices");
    const missing = setup();
    await begin(missing.hook.result);
    expect(missing.hook.result.current.state).toEqual({ kind: "notice", reason: "no-device" });
    vi.stubGlobal("MediaRecorder", undefined);
    const noRecorder = setup();
    await begin(noRecorder.hook.result);
    expect(noRecorder.hook.result.current.state).toEqual({ kind: "notice", reason: "no-device" });
  });

  it("opens the system settings through the API", () => {
    const { hook, api } = setup();
    act(() => hook.result.current.openSettings());
    expect(api.microphoneSettingsOpened).toBe(1);
  });
});

describe("cancelling", () => {
  it("drops a recording under five seconds at once and releases the microphone", async () => {
    const { hook, api } = setup();
    await begin(hook.result);
    await advance(CONFIRM_AFTER_MS - 1000);
    act(() => hook.result.current.requestCancel());
    expect(hook.result.current.state.kind).toBe("idle");
    expect(stream.track.stopped).toBe(true);
    expect(api.sentVoices).toHaveLength(0);
  });

  it("asks first after five seconds, keeps recording while it asks, and Keep goes on", async () => {
    const { hook } = setup();
    await begin(hook.result);
    await advance(CONFIRM_AFTER_MS + 1000);
    act(() => hook.result.current.requestCancel());
    expect(hook.result.current.state.kind).toBe("confirming");
    await advance(2000);
    expect(hook.result.current.state.kind).toBe("confirming");
    expect(stream.track.stopped).toBe(false);
    act(() => hook.result.current.keepRecording());
    expect(hook.result.current.state.kind).toBe("recording");
    expect(FakeRecorder.last?.state).toBe("recording");
  });

  it("Discard in the question drops the recording and stops every track", async () => {
    const { hook, api } = setup();
    await begin(hook.result);
    await advance(CONFIRM_AFTER_MS + 1000);
    act(() => hook.result.current.requestCancel());
    act(() => hook.result.current.discard());
    expect(hook.result.current.state.kind).toBe("idle");
    expect(stream.track.stopped).toBe(true);
    expect(api.sentVoices).toHaveLength(0);
  });
});

describe("sending", () => {
  it("sends the bytes to the chat with the recorder's clock and the reply, then goes idle with the tracks stopped", async () => {
    const { hook, api, onSent, props } = setup();
    const question = await api.sendMessage(INBOX_ID, "the question");
    hook.rerender({ ...props, replyToId: question.id });
    await begin(hook.result);
    FakeRecorder.last?.chunk(1000);
    await advance(40_000);
    await finish(hook.result);
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]).toMatchObject({ chatId: INBOX_ID, bytes: 1016, replyToId: question.id });
    expect(api.sentVoices[0]?.durationMs).toBeGreaterThanOrEqual(40_000);
    expect(onSent).toHaveBeenCalledTimes(1);
    expect(hook.result.current.state.kind).toBe("idle");
    expect(stream.track.stopped).toBe(true);
  });

  it("under a second is not saved: 'Too short to save'", async () => {
    const { hook, api } = setup();
    await begin(hook.result);
    await advance(500);
    await finish(hook.result);
    expect(api.sentVoices).toHaveLength(0);
    expect(hook.result.current.state).toEqual({ kind: "notice", reason: "too-short" });
    expect(stream.track.stopped).toBe(true);
  });

  it("a note goes to the chat it was started in, even after the person opens another", async () => {
    const { hook, api, props } = setup();
    await begin(hook.result);
    await advance(3000);
    hook.rerender({ ...props, chatId: OTHER_CHAT });
    await finish(hook.result);
    expect(api.sentVoices[0]?.chatId).toBe(INBOX_ID);
    expect(hook.result.current.state.kind).toBe("idle");
  });

  it("sends by itself at 30:00", async () => {
    const { hook, api } = setup();
    await begin(hook.result);
    await advance(MAX_RECORDING_MS + 1000);
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]?.durationMs).toBeLessThanOrEqual(1_805_000);
    expect(stream.track.stopped).toBe(true);
  });

  it("sends by itself at 48 MiB, long before 30:00", async () => {
    const { hook, api } = setup();
    await begin(hook.result);
    await advance(60_000);
    const size = 8 * 1024 * 1024;
    for (let i = 0; i < MAX_RECORDING_BYTES / size; i += 1) FakeRecorder.last?.chunk(size);
    await advance(10);
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]?.bytes).toBeGreaterThanOrEqual(MAX_RECORDING_BYTES);
    expect(api.sentVoices[0]?.durationMs).toBeLessThan(MAX_RECORDING_MS);
    expect(stream.track.stopped).toBe(true);
  });

  it("a failed save keeps the bytes: Retry sends the same recording, Discard drops it", async () => {
    const { hook, api } = setup();
    await begin(hook.result);
    await advance(5000);
    api.failNext("sendVoice", "disk_full");
    await finish(hook.result);
    expect(hook.result.current.state).toMatchObject({ kind: "send-failed", error: "disk_full" });
    expect(stream.track.stopped).toBe(true);
    await act(async () => {
      hook.result.current.retry();
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(api.sentVoices).toHaveLength(1);
    expect(hook.result.current.state.kind).toBe("idle");

    await begin(hook.result);
    await advance(5000);
    api.failNext("sendVoice", "disk_full");
    await finish(hook.result);
    act(() => hook.result.current.discard());
    expect(hook.result.current.state.kind).toBe("idle");
    expect(api.sentVoices).toHaveLength(1);
  });
});

describe("what a second try cannot change is not offered again (one test per code)", () => {
  async function recordAndFail(code: Parameters<MockApi["failNext"]>[1], options: Parameters<typeof setup>[0] = {}) {
    const made = setup(options);
    await begin(made.hook.result);
    await advance(5000);
    made.api.failNext("sendVoice", code);
    await finish(made.hook.result);
    return made;
  }

  it("a recording whose auto-send tick came late is sent as 30:00, not refused as too long", async () => {
    const { hook, api } = setup();
    await begin(hook.result);
    const start = performance.now();
    // The tick that should have sent the note at 30:00 did not run for ten more seconds (a hidden window, a swapped-out VM).
    vi.spyOn(performance, "now").mockReturnValue(start + MAX_RECORDING_MS + 10_000);
    await finish(hook.result);
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]?.durationMs).toBe(MAX_RECORDING_MS);
    expect(hook.result.current.state.kind).toBe("idle");
    vi.restoreAllMocks();
  });

  it("voice_too_long ends the recording with a line above the field, no Retry", async () => {
    const { hook, api } = await recordAndFail("voice_too_long");
    expect(hook.result.current.state).toEqual({ kind: "notice", reason: "too-long" });
    expect(api.sentVoices).toHaveLength(0);
    expect(stream.track.stopped).toBe(true);
  });

  it("voice_too_short from Rust ends the recording the same way", async () => {
    const { hook } = await recordAndFail("voice_too_short");
    expect(hook.result.current.state).toEqual({ kind: "notice", reason: "too-short" });
  });

  it("reply_target_not_found sends the note once more with no reply, so the recording is kept", async () => {
    const { hook, api } = setup();
    const question = await api.sendMessage(INBOX_ID, "the question");
    hook.rerender({ chatId: INBOX_ID, replyToId: question.id });
    await begin(hook.result);
    await advance(5000);
    api.failNext("sendVoice", "reply_target_not_found");
    await finish(hook.result);
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]).toMatchObject({ chatId: INBOX_ID, replyToId: null });
    expect(hook.result.current.state.kind).toBe("idle");
  });

  it("reply_target_not_found without a reply to drop is Discard only", async () => {
    const { hook } = await recordAndFail("reply_target_not_found");
    expect(hook.result.current.state).toMatchObject({ kind: "send-failed", error: "reply_target_not_found", retryable: false });
  });

  it.each(["chat_not_found", "attachment_unreadable"] as const)("%s leaves Discard only", async (code) => {
    const { hook, api } = await recordAndFail(code);
    expect(hook.result.current.state).toMatchObject({ kind: "send-failed", error: code, retryable: false });
    act(() => hook.result.current.discard());
    expect(hook.result.current.state.kind).toBe("idle");
    expect(api.sentVoices).toHaveLength(0);
  });

  it("a full disk keeps the bytes and offers Retry", async () => {
    const { hook } = await recordAndFail("disk_full");
    expect(hook.result.current.state).toMatchObject({ kind: "send-failed", error: "disk_full", retryable: true });
  });
});

describe("a lost device", () => {
  it("whose recording cannot be read does not leave the bar stuck", async () => {
    const { hook } = setup();
    await begin(hook.result);
    await advance(5000);
    vi.spyOn(Blob.prototype, "arrayBuffer").mockRejectedValue(new Error("read failed"));
    await act(async () => {
      stream.track.dispatchEvent(new Event("ended"));
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(hook.result.current.state).toEqual({ kind: "notice", reason: "busy" });
    vi.restoreAllMocks();
  });

  it("keeps what was recorded and offers Send", async () => {
    const { hook, api } = setup();
    await begin(hook.result);
    FakeRecorder.last?.chunk(500);
    await advance(12_000);
    await act(async () => {
      stream.track.dispatchEvent(new Event("ended"));
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(hook.result.current.state).toMatchObject({ kind: "lost" });
    expect(api.sentVoices).toHaveLength(0);
    await finish(hook.result);
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]?.durationMs).toBeGreaterThanOrEqual(12_000);
    expect(api.sentVoices[0]?.bytes).toBe(516);
  });

  it("Discard after a lost device drops it", async () => {
    const { hook, api } = setup();
    await begin(hook.result);
    await advance(8000);
    await act(async () => {
      stream.track.dispatchEvent(new Event("ended"));
      await vi.advanceTimersByTimeAsync(10);
    });
    act(() => hook.result.current.discard());
    expect(hook.result.current.state.kind).toBe("idle");
    expect(api.sentVoices).toHaveLength(0);
    expect(stream.track.stopped).toBe(true);
  });
});

describe("the window going away", () => {
  it("a running recording is lost and the microphone released when the hook unmounts", async () => {
    const { hook, api } = setup();
    await begin(hook.result);
    hook.unmount();
    expect(stream.track.stopped).toBe(true);
    expect(api.sentVoices).toHaveLength(0);
  });
});

describe("privacy", () => {
  it("a device label never reaches the API, the state or the console", async () => {
    const spies = (["log", "info", "warn", "error", "debug"] as const).map((method) => vi.spyOn(console, method));
    const { hook, api } = setup();
    await begin(hook.result);
    await advance(5000);
    expect(JSON.stringify(hook.result.current.state)).not.toContain(LABEL);
    await finish(hook.result);
    expect(JSON.stringify(api.sentVoices)).not.toContain(LABEL);
    expect(api.calls.join("\n")).not.toContain(LABEL);
    for (const spy of spies) expect(JSON.stringify(spy.mock.calls)).not.toContain(LABEL);
    refuse("NotAllowedError");
    await begin(hook.result);
    expect(JSON.stringify(hook.result.current.state)).not.toContain(LABEL);
  });
});
