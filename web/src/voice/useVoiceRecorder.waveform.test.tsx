import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../api/KinigeApi";
import { MockApi } from "../api/MockApi";
import { act, renderHook } from "../test/render";
import { LEVEL_PERIOD_MS } from "./recorderSession";
import { useVoiceRecorder } from "./useVoiceRecorder";

// The recording's waveform (slice 7a, round 2): the recorder reads an analyser every 50 ms; the levels draw the live waveform and become the
// 64 peaks that go with the note. A fake `MediaDevices`, `MediaRecorder` and `AudioContext` run the real pipeline with no device.

class FakeTrack extends EventTarget {
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
  state: "inactive" | "recording" = "inactive";
  static isTypeSupported() {
    return true;
  }
  start() {
    this.state = "recording";
  }
  stop() {
    this.state = "inactive";
    const event = new Event("dataavailable") as Event & { data: Blob };
    event.data = new Blob([new Uint8Array(32).fill(7)]);
    this.dispatchEvent(event);
    this.dispatchEvent(new Event("stop"));
  }
}

/** What the analyser reads: a constant amplitude in 0..1 (the deviation of the samples from the middle), set by the test. */
let amplitude = 0;
let contexts: { closed: boolean }[] = [];
let analyserAvailable = true;

class FakeAnalyser {
  fftSize = 0;
  getByteTimeDomainData(samples: Uint8Array) {
    // Alternating samples: the root mean square is exactly the amplitude.
    samples.forEach((_, index) => (samples[index] = 128 + (index % 2 === 0 ? 1 : -1) * Math.round(amplitude * 128)));
  }
}

class FakeContext {
  readonly state = { closed: false };
  constructor() {
    if (!analyserAvailable) throw new Error("no audio");
    contexts.push(this.state);
  }
  resume() {
    return Promise.resolve();
  }
  createAnalyser() {
    return new FakeAnalyser();
  }
  createMediaStreamSource() {
    return { connect: () => undefined };
  }
  close() {
    this.state.closed = true;
    return Promise.resolve();
  }
}

beforeEach(() => {
  vi.useFakeTimers();
  amplitude = 0;
  contexts = [];
  analyserAvailable = true;
  Object.defineProperty(navigator, "mediaDevices", { value: { getUserMedia: () => Promise.resolve(new FakeStream()) }, configurable: true });
  vi.stubGlobal("MediaRecorder", FakeRecorder);
  vi.stubGlobal("AudioContext", FakeContext);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(navigator, "mediaDevices");
});

function setup() {
  const api = new MockApi();
  const hook = renderHook(() => useVoiceRecorder({ api, chatId: INBOX_ID, replyToId: null, onSent: vi.fn() }));
  return { api, hook };
}

async function record(hook: ReturnType<typeof setup>["hook"], script: [amplitude: number, ms: number][]) {
  await act(async () => {
    hook.result.current.start();
    await Promise.resolve();
  });
  for (const [level, ms] of script) {
    amplitude = level;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(ms);
    });
  }
}

async function send(hook: ReturnType<typeof setup>["hook"]) {
  await act(async () => {
    hook.result.current.send();
    await vi.advanceTimersByTimeAsync(10);
  });
}

describe("the waveform of a recording", () => {
  it("is read from the analyser every 50 ms and sent with the note as 64 peaks, the loudest being 255", async () => {
    const { api, hook } = setup();
    await record(hook, [
      [0.04, 3000],
      [0.16, 3000],
    ]);
    await send(hook);
    const sent = api.sentVoices[0];
    expect(sent?.waveform).toHaveLength(64);
    const peaks = Array.from(sent?.waveform ?? []);
    expect(Math.max(...peaks)).toBe(255);
    // The first half is the quiet part (a quarter of the amplitude, half of the height after the square root), the second the loud part.
    expect(peaks[10]).toBeGreaterThan(100);
    expect(peaks[10]).toBeLessThan(140);
    expect(peaks[50]).toBe(255);
  });

  it("the live levels are the newest ones, oldest first, with the time of the newest", async () => {
    const { hook } = setup();
    await record(hook, [[0.25, 1000]]);
    const live = hook.result.current.levels(10);
    expect(live.levels).toHaveLength(10);
    // sqrt(0.25) = 0.5, as a byte and back.
    expect(live.levels.every((level) => Math.abs(level - 0.5) < 0.01)).toBe(true);
    expect(live.at).toBeGreaterThan(0);
    expect(hook.result.current.levels(1_000_000).levels.length).toBeGreaterThanOrEqual(15);
    expect(hook.result.current.levels(-5).levels).toEqual([]);
    expect(hook.result.current.levels(0).levels).toEqual([]);
  });

  it("a timer that fires late keeps the time axis: the level holds for the missed periods", async () => {
    const { api, hook } = setup();
    await record(hook, [[0.1, 1500]]);
    const before = hook.result.current.levels(100_000).levels.length;
    // One long gap (a hidden window runs timers about once a second): a single tick after 2 s instead of forty.
    amplitude = 0.4;
    const base = performance.now();
    vi.spyOn(performance, "now").mockReturnValue(base + 2000);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(LEVEL_PERIOD_MS);
    });
    const after = hook.result.current.levels(100_000).levels.length;
    expect(after - before).toBe(40);
    await send(hook);
    const peaks = Array.from(api.sentVoices[0]?.waveform ?? []);
    expect(peaks.length).toBe(64);
    expect(Math.max(...peaks)).toBe(255);
    // The loud gap is more than half of the recording: it fills the second half of the bars.
    expect(peaks[63]).toBe(255);
    expect(peaks[5]).toBeLessThan(200);
  });

  it("a recording in silence sends no peaks (the player decodes the file later), and the note is still saved", async () => {
    const { api, hook } = setup();
    await record(hook, [[0, 2000]]);
    await send(hook);
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]?.waveform).toBeNull();
  });

  it("a web view with no AudioContext, or one that throws, still records and sends the note, with no peaks", async () => {
    vi.stubGlobal("AudioContext", undefined);
    const none = setup();
    await record(none.hook, [[0.3, 2000]]);
    await send(none.hook);
    expect(none.api.sentVoices[0]?.waveform).toBeNull();
    expect(none.hook.result.current.levels(5).levels).toEqual([]);

    vi.stubGlobal("AudioContext", FakeContext);
    analyserAvailable = false;
    const broken = setup();
    await record(broken.hook, [[0.3, 2000]]);
    await send(broken.hook);
    expect(broken.api.sentVoices).toHaveLength(1);
    expect(broken.api.sentVoices[0]?.waveform).toBeNull();
  });

  it("the analyser's audio context is closed on every way out (Send and Discard)", async () => {
    const sent = setup();
    await record(sent.hook, [[0.3, 2000]]);
    await send(sent.hook);
    const discarded = setup();
    await record(discarded.hook, [[0.3, 1000]]);
    act(() => discarded.hook.result.current.requestCancel());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(contexts.length).toBe(2);
    expect(contexts.every((context) => context.closed)).toBe(true);
  });

  it("a note kept after a lost device carries its peaks too", async () => {
    const { api, hook } = setup();
    let stream: FakeStream | null = null;
    Object.defineProperty(navigator, "mediaDevices", {
      value: { getUserMedia: () => Promise.resolve((stream = new FakeStream())) },
      configurable: true,
    });
    await record(hook, [[0.2, 2000]]);
    await act(async () => {
      stream?.track.dispatchEvent(new Event("ended"));
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(hook.result.current.state.kind).toBe("lost");
    await send(hook);
    expect(api.sentVoices[0]?.waveform).toHaveLength(64);
  });
});
