import { afterEach, describe, expect, it, vi } from "vitest";
import { decodePeaks, levelOfRms, peaksFromRms, peaksFromSamples, peaksToHex, WAVEFORM_CURVE, WAVEFORM_PEAKS } from "./waveform";

afterEach(() => vi.unstubAllGlobals());

describe("peaksToHex", () => {
  it("writes two hex digits per peak", () => {
    expect(peaksToHex(Uint8Array.from([0, 1, 15, 16, 255]))).toBe("00010f10ff");
    expect(peaksToHex(new Uint8Array(0))).toBe("");
    expect(peaksToHex(new Uint8Array(WAVEFORM_PEAKS)).length).toBe(128);
  });
});

describe("levelOfRms", () => {
  it("spreads quiet levels with a square root and clamps garbage", () => {
    expect(levelOfRms(0.25)).toBe(0.5);
    expect(levelOfRms(0)).toBe(0);
    expect(levelOfRms(1)).toBe(1);
    expect(levelOfRms(4)).toBe(1);
    expect(levelOfRms(-1)).toBe(0);
    expect(levelOfRms(Number.NaN)).toBe(0);
    expect(levelOfRms(Number.POSITIVE_INFINITY)).toBe(0);
  });
});

describe("peaksFromRms", () => {
  const curve = (share: number) => Math.round(share ** WAVEFORM_CURVE * 255);

  it("gives exactly 64 peaks; a bar is the plain RMS of its share, the loudest bar is 255 and the mild curve comes after", () => {
    // One loud window (0.5) among 640 quiet ones (0.1): bar 10 holds windows 100 to 109 and the loud one.
    const windows = Array.from({ length: 640 }, (_, i) => (i === 100 ? 0.5 : 0.1));
    const peaks = peaksFromRms(windows);
    expect(peaks).toHaveLength(WAVEFORM_PEAKS);
    expect(Math.max(...(peaks ?? []))).toBe(255);
    expect(peaks?.[10]).toBe(255);
    // The other bars: 0.1 against the RMS of ten windows with one 0.5 in them, sqrt((9 * 0.01 + 0.25) / 10) = 0.186.
    expect(peaks?.[0]).toBe(curve(0.1 / Math.sqrt((9 * 0.01 + 0.25) / 10)));
    expect(peaks?.[63]).toBe(peaks?.[0]);
  });

  it("is the RMS of the bar, not its loudest window: one spike among quiet windows does not make the bar as loud as the spike", () => {
    const peaks = peaksFromRms([0.1, 0.1, 0.1, 0.9, 0.1, 0.1, 0.1, 0.1], 2);
    // Bar 0: sqrt((3 * 0.01 + 0.81) / 4) = 0.458; bar 1: 0.1.
    expect(peaks?.[0]).toBe(255);
    expect(peaks?.[1]).toBe(curve(0.1 / 0.458));
  });

  it("keeps dynamics: quiet syllables stay clearly lower than loud ones", () => {
    const windows = Array.from({ length: 640 }, (_, i) => (Math.floor(i / 10) % 2 === 0 ? 0.2 : 0.05));
    const peaks = Array.from(peaksFromRms(windows) ?? []);
    expect(Math.max(...peaks)).toBe(255);
    expect(Math.min(...peaks)).toBeLessThan(110);
  });

  it("stretches a short series over all bars and never skips a window", () => {
    const peaks = peaksFromRms([0.2, 0.4]);
    expect(peaks).toHaveLength(WAVEFORM_PEAKS);
    expect(peaks?.[0]).toBe(curve(0.5));
    expect(peaks?.[63]).toBe(255);
    const one = peaksFromRms([0.3]);
    expect(one).toHaveLength(WAVEFORM_PEAKS);
    expect(new Set(one)).toEqual(new Set([255]));
  });

  it("is null for nothing to draw: empty, silent or a count that makes no sense", () => {
    expect(peaksFromRms([])).toBeNull();
    expect(peaksFromRms([0, 0, 0])).toBeNull();
    expect(peaksFromRms([0.5], 0)).toBeNull();
    expect(peaksFromRms([0.5], -3)).toBeNull();
  });

  it("treats NaN and out-of-range windows as quiet or loud and never throws", () => {
    const peaks = peaksFromRms([Number.NaN, 0.5, Number.NaN, 2], 4);
    expect(peaks).toHaveLength(4);
    expect(peaks?.every((peak) => peak >= 0 && peak <= 255)).toBe(true);
    expect(peaksFromRms([Number.NaN, Number.NaN])).toBeNull();
  });

  it("a quiet recording still reaches the full height", () => {
    const quiet = peaksFromRms(Array.from({ length: 200 }, (_, i) => 0.01 + (i % 10) * 0.001));
    expect(Math.max(...(quiet ?? []))).toBe(255);
  });
});

describe("peaksFromSamples", () => {
  it("makes the same peaks for a tone that gets louder", () => {
    const rate = 8000;
    const samples = new Float32Array(rate * 4);
    for (let i = 0; i < samples.length; i += 1) samples[i] = Math.sin((i / rate) * 440 * Math.PI * 2) * (i / samples.length);
    const peaks = peaksFromSamples(samples, rate);
    expect(peaks).toHaveLength(WAVEFORM_PEAKS);
    expect(peaks?.[63]).toBe(255);
    expect((peaks?.[0] ?? 255) < (peaks?.[32] ?? 0)).toBe(true);
    expect((peaks?.[32] ?? 255) < (peaks?.[63] ?? 0)).toBe(true);
  });

  it("is null for silence and for no samples", () => {
    expect(peaksFromSamples(new Float32Array(8000), 8000)).toBeNull();
    expect(peaksFromSamples(new Float32Array(0), 8000)).toBeNull();
  });

  it("copes with a sample rate that makes a window of zero samples", () => {
    expect(peaksFromSamples(Float32Array.from([0.5, 0.5, 0.5]), 0)).toHaveLength(WAVEFORM_PEAKS);
  });
});

describe("decodePeaks", () => {
  function fakeDecoder(buffer: { length: number; sampleRate: number; channels: Float32Array[] } | Error) {
    class Fake {
      constructor(
        readonly channels: number,
        readonly length: number,
        readonly sampleRate: number,
      ) {}
      decodeAudioData(): Promise<unknown> {
        if (buffer instanceof Error) return Promise.reject(buffer);
        return Promise.resolve({
          length: buffer.length,
          sampleRate: buffer.sampleRate,
          numberOfChannels: buffer.channels.length,
          getChannelData: (index: number) => buffer.channels[index],
        });
      }
    }
    vi.stubGlobal("OfflineAudioContext", Fake);
  }

  it("decodes at 8 kHz and reads every channel", async () => {
    const seen: number[] = [];
    class Spy {
      constructor(_channels: number, _length: number, rate: number) {
        seen.push(rate);
      }
      decodeAudioData() {
        const left = new Float32Array(8000 * 2);
        const right = new Float32Array(8000 * 2);
        // The first second is silent on the left and loud on the right; the second is the other way round.
        for (let i = 0; i < 8000; i += 1) right[i] = Math.sin(i * 0.2) * 0.5;
        for (let i = 8000; i < 16000; i += 1) left[i] = Math.sin(i * 0.2) * 0.25;
        return Promise.resolve({ length: 16000, sampleRate: 8000, numberOfChannels: 2, getChannelData: (n: number) => (n === 0 ? left : right) });
      }
    }
    vi.stubGlobal("OfflineAudioContext", Spy);
    const peaks = await decodePeaks(new ArrayBuffer(8));
    expect(seen).toEqual([8000]);
    expect(peaks).toHaveLength(WAVEFORM_PEAKS);
    expect(Math.max(...peaks)).toBe(255);
    // The loud half comes from the right channel, the quieter half (half the amplitude) from the left one.
    expect(peaks[10]).toBeGreaterThan(230);
    expect(peaks[50]).toBeGreaterThan(100);
    expect(peaks[50]).toBeLessThan(200);
  });

  it("rejects when the web view cannot decode the file, when the file is silent and when there is no decoder", async () => {
    fakeDecoder(new Error("EncodingError"));
    await expect(decodePeaks(new ArrayBuffer(8))).rejects.toThrow();
    fakeDecoder({ length: 800, sampleRate: 8000, channels: [new Float32Array(800)] });
    await expect(decodePeaks(new ArrayBuffer(8))).rejects.toThrow("silent");
    vi.unstubAllGlobals();
    await expect(decodePeaks(new ArrayBuffer(8))).rejects.toThrow("no decoder");
  });
});
