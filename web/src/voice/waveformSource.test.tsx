import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "../test/render";
import { FakeObserver } from "../test/fakeObserver";
import { demoPeaks, demoVoice } from "../ui/attachments/demo";
import { VoicePlayer } from "../ui/voice/VoicePlayer";
import {
  ensurePeaks,
  MAX_DECODE_BYTES,
  MAX_DECODE_MS,
  resetWaveformSession,
  WaveformSourceContext,
  type WaveformSource,
} from "./waveformSource";

/** A tone that gets louder: decodes to peaks that rise. */
function stubDecoder(failWith?: Error) {
  class Fake {
    decodeAudioData() {
      if (failWith) return Promise.reject(failWith);
      const samples = new Float32Array(8000 * 3);
      for (let i = 0; i < samples.length; i += 1) samples[i] = Math.sin(i * 0.3) * (i / samples.length);
      return Promise.resolve({ length: samples.length, sampleRate: 8000, numberOfChannels: 1, getChannelData: () => samples });
    }
  }
  vi.stubGlobal("OfflineAudioContext", Fake);
}

function source(overrides: Partial<WaveformSource> = {}) {
  const read = vi.fn<WaveformSource["read"]>(() => Promise.resolve(new ArrayBuffer(16)));
  const save = vi.fn<WaveformSource["save"]>(() => Promise.resolve());
  const value: WaveformSource = { read, save, ...overrides };
  return { read: value.read as typeof read, save: value.save as typeof save, value };
}

beforeEach(() => {
  resetWaveformSession();
  stubDecoder();
  vi.spyOn(HTMLMediaElement.prototype, "canPlayType").mockImplementation(() => "maybe");
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  FakeObserver.all = [];
});

const voice = (id: string, extra: Parameters<typeof demoVoice>[2] = {}) => demoVoice(id, 12_000, { byteSize: 6_000, ...extra });

describe("ensurePeaks", () => {
  it("reads, decodes and saves the peaks of a note", async () => {
    const { value, read, save } = source();
    const peaks = await ensurePeaks(value, voice("a"));
    expect(peaks).toHaveLength(64);
    expect(Math.max(...(peaks ?? []))).toBe(255);
    expect(read).toHaveBeenCalledWith("a");
    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0]?.[0]).toBe("a");
    expect(save.mock.calls[0]?.[1]).toHaveLength(64);
  });

  it("decodes one note at a time, in the order asked", async () => {
    let release: (bytes: ArrayBuffer) => void = () => undefined;
    const first = new Promise<ArrayBuffer>((resolve) => (release = resolve));
    const read = vi.fn((id: string) => (id === "a" ? first : Promise.resolve(new ArrayBuffer(16))));
    const { value, save } = source({ read });
    const a = ensurePeaks(value, voice("a"));
    const b = ensurePeaks(value, voice("b"));
    await Promise.resolve();
    await Promise.resolve();
    expect(read.mock.calls.map((call) => call[0])).toEqual(["a"]);
    release(new ArrayBuffer(16));
    await Promise.all([a, b]);
    expect(read.mock.calls.map((call) => call[0])).toEqual(["a", "b"]);
    expect(save.mock.calls.map((call) => call[0])).toEqual(["a", "b"]);
  });

  it("asks once for a note that is already being decoded, and not at all for one decoded earlier", async () => {
    const { value, read, save } = source();
    const [one, two] = await Promise.all([ensurePeaks(value, voice("a")), ensurePeaks(value, voice("a"))]);
    expect(one).toEqual(two);
    await ensurePeaks(value, voice("a"));
    expect(read).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("a file over 16 MiB, or a note over 20 minutes, is never read: the empty marker is saved", async () => {
    const { value, read, save } = source();
    const big = await ensurePeaks(value, voice("big", { byteSize: MAX_DECODE_BYTES + 1 }));
    const long = await ensurePeaks(value, demoVoice("long", MAX_DECODE_MS + 1, { byteSize: 1000 }));
    expect(big).toHaveLength(0);
    expect(long).toHaveLength(0);
    expect(read).not.toHaveBeenCalled();
    expect(save.mock.calls.map((call) => [call[0], call[1]?.length])).toEqual([
      ["big", 0],
      ["long", 0],
    ]);
  });

  it("a file at exactly 16 MiB is decoded", async () => {
    const { value, read } = source();
    await ensurePeaks(value, voice("edge", { byteSize: MAX_DECODE_BYTES }));
    expect(read).toHaveBeenCalledTimes(1);
  });

  it("a file this web view cannot decode gets the empty marker, so it is never tried again", async () => {
    stubDecoder(new Error("EncodingError"));
    const { value, save } = source();
    const peaks = await ensurePeaks(value, voice("ogg"));
    expect(peaks).toHaveLength(0);
    expect(save.mock.calls[0]?.[1]).toHaveLength(0);
    const again = await ensurePeaks(value, voice("ogg"));
    expect(again).toHaveLength(0);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("a read that fails (the file is gone for now) saves nothing and is tried again the next time", async () => {
    const read = vi.fn().mockRejectedValueOnce(new Error("busy")).mockResolvedValue(new ArrayBuffer(16));
    const { value, save } = source({ read });
    expect(await ensurePeaks(value, voice("a"))).toBeNull();
    expect(save).not.toHaveBeenCalled();
    expect(await ensurePeaks(value, voice("a"))).toHaveLength(64);
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("a save that fails still shows the peaks", async () => {
    const save = vi.fn().mockRejectedValue(new Error("disk"));
    const { value } = source({ save });
    expect(await ensurePeaks(value, voice("a"))).toHaveLength(64);
  });

  it("one failing note does not stop the queue", async () => {
    const read = vi.fn((id: string) => (id === "a" ? Promise.reject(new Error("x")) : Promise.resolve(new ArrayBuffer(16))));
    const { value } = source({ read });
    const [a, b] = await Promise.all([ensurePeaks(value, voice("a")), ensurePeaks(value, voice("b"))]);
    expect(a).toBeNull();
    expect(b).toHaveLength(64);
  });
});

describe("a player without peaks", () => {
  function show(attachment: ReturnType<typeof voice>, value: WaveformSource | null) {
    return render(
      <WaveformSourceContext.Provider value={value}>
        <VoicePlayer attachment={attachment} onOpenFile={vi.fn()} />
      </WaveformSourceContext.Provider>,
      { language: "en" },
    );
  }
  const firstBar = (container: HTMLElement) => container.querySelector<HTMLElement>(".voice-player__bar")?.style.getPropertyValue("--peak");

  beforeEach(() => vi.stubGlobal("IntersectionObserver", FakeObserver));

  it("waits until it is on screen, then decodes once and draws the peaks without changing size", async () => {
    const { value, read, save } = source();
    const { container } = show(voice("a"), value);
    const wave = container.querySelector(".voice-player__wave") as HTMLElement;
    const placeholder = firstBar(container);
    expect(read).not.toHaveBeenCalled();
    expect(FakeObserver.all).toHaveLength(1);
    act(() => FakeObserver.all[0]?.fire(wave));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(firstBar(container)).not.toBe(placeholder));
    expect(container.querySelectorAll(".voice-player__bar")).toHaveLength(128);
    expect(read).toHaveBeenCalledTimes(1);
  });

  it("a note whose row has peaks never decodes and never watches the screen", () => {
    const { value, read } = source();
    show(voice("a", { waveform: demoPeaks(1) }), value);
    expect(FakeObserver.all).toHaveLength(0);
    expect(read).not.toHaveBeenCalled();
  });

  it("without a source (stories, tests) it keeps the placeholder and watches nothing", () => {
    show(voice("a"), null);
    expect(FakeObserver.all).toHaveLength(0);
  });

  it("an unplayable or missing note never decodes", () => {
    const { value } = source();
    show(voice("a", { state: "missing" }), value);
    show(voice("b", { byteSize: 70 * 1024 * 1024 }), value);
    expect(FakeObserver.all).toHaveLength(0);
    expect(screen.getByText("File is missing")).toBeInTheDocument();
  });

  it("scrolling away before it shows stops the watch", () => {
    const { value } = source();
    const { unmount } = show(voice("a"), value);
    expect(FakeObserver.all).toHaveLength(1);
    unmount();
    expect(FakeObserver.all).toHaveLength(0);
  });

  it("a player that shows again after a scroll finds the peaks decoded this session", async () => {
    const { value, read } = source();
    const first = show(voice("a"), value);
    act(() => FakeObserver.all[0]?.fire(first.container.querySelector(".voice-player__wave") as HTMLElement));
    await waitFor(() => expect(first.container.querySelector(".voice-player__bar")).not.toBeNull());
    await waitFor(() => expect(read).toHaveBeenCalledTimes(1));
    await act(() => Promise.resolve());
    first.unmount();
    const second = show(voice("a"), value);
    expect(FakeObserver.all).toHaveLength(0);
    expect(read).toHaveBeenCalledTimes(1);
    expect(second.container.querySelectorAll(".voice-player__bar")).toHaveLength(128);
  });
});
