import { afterEach, describe, expect, it, vi } from "vitest";
import { decodePeaks, levelOfRms, peaksFromLevels, peaksFromSamples, peaksToHex, WAVEFORM_PEAKS } from "./waveform";

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

describe("peaksFromLevels", () => {
  it("gives exactly 64 peaks, the loudest is 255 and each bar is the loudest level of its share", () => {
    const levels = Array.from({ length: 640 }, (_, i) => (i === 100 ? 0.5 : 0.1));
    const peaks = peaksFromLevels(levels);
    expect(peaks).toHaveLength(WAVEFORM_PEAKS);
    expect(Math.max(...(peaks ?? []))).toBe(255);
    // Level 100 is in the share of bar 10 (levels 100 to 109); the other bars hold 0.1 of 0.5.
    expect(peaks?.[10]).toBe(255);
    expect(peaks?.[0]).toBe(51);
    expect(peaks?.[63]).toBe(51);
  });

  it("stretches a short series over all bars and never skips a level", () => {
    const peaks = peaksFromLevels([0.2, 0.4]);
    expect(peaks).toHaveLength(WAVEFORM_PEAKS);
    expect(peaks?.[0]).toBe(128);
    expect(peaks?.[63]).toBe(255);
    const one = peaksFromLevels([0.3]);
    expect(one).toHaveLength(WAVEFORM_PEAKS);
    expect(new Set(one)).toEqual(new Set([255]));
  });

  it("is null for nothing to draw: empty, silent or a count that makes no sense", () => {
    expect(peaksFromLevels([])).toBeNull();
    expect(peaksFromLevels([0, 0, 0])).toBeNull();
    expect(peaksFromLevels([0.5], 0)).toBeNull();
    expect(peaksFromLevels([0.5], -3)).toBeNull();
  });

  it("treats NaN and out-of-range levels as quiet or loud and never throws", () => {
    const peaks = peaksFromLevels([Number.NaN, 0.5, Number.NaN, 2], 4);
    expect(peaks).toHaveLength(4);
    expect(peaks?.every((peak) => peak >= 0 && peak <= 255)).toBe(true);
    expect(peaksFromLevels([Number.NaN, Number.NaN])).toBeNull();
  });

  it("a quiet recording still reaches the full height", () => {
    const quiet = peaksFromLevels(Array.from({ length: 200 }, (_, i) => 0.01 + (i % 10) * 0.001));
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
