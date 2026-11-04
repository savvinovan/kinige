// The waveform of a voice note (slice 7a, ADR-0015): a fixed number of peaks, each 0 to 255, the loudest part of the note being 255. The
// recorder collects them from its analyser while recording; for a note without peaks (an import, an older version) the player decodes
// the file once in the web view and saves what it found. Pure functions here: no React, no IPC.

/** How many peaks a waveform has: the same number as `kinige_domain::attachment::WAVEFORM_PEAKS`, and the number of bars the player draws. */
export const WAVEFORM_PEAKS = 64;

/** The decoder works at this rate: a voice note needs no more, and a 30 minute note then takes 58 MB of samples instead of 320 MB. */
const DECODE_SAMPLE_RATE = 8000;
/** The decoder looks at the signal in windows of this length; a bar is the loudest window of its share of the note. */
const WINDOW_MS = 50;

/** Reads `0x00`..`0xff` of every peak as two hex digits, the form the wire (a header, a command argument) carries. */
export function peaksToHex(peaks: Uint8Array): string {
  let text = "";
  for (const peak of peaks) text += peak.toString(16).padStart(2, "0");
  return text;
}

/** What a level of the recorder's analyser (the root mean square of a window, 0 to 1) looks like on the screen: speech sits low, so the root spreads it. */
export function levelOfRms(rms: number): number {
  return Math.sqrt(Math.min(1, Math.max(0, Number.isFinite(rms) ? rms : 0)));
}

/**
 * Turns a series of levels (0 to 1, oldest first, any length) into [`WAVEFORM_PEAKS`] peaks: each bar is the loudest level of its share
 * of the series, and the loudest bar is 255, so a quiet voice still fills the height. `null` for an empty series, or a series that is
 * silent all the way through (nothing to draw but a flat line, which the placeholder already is).
 */
export function peaksFromLevels(levels: ArrayLike<number>, count: number = WAVEFORM_PEAKS): Uint8Array | null {
  if (levels.length === 0 || count <= 0) return null;
  const maxima = new Float64Array(count);
  for (let bar = 0; bar < count; bar += 1) {
    // The share of bar `bar`: at least one level, so a short series is stretched, never skipped.
    const from = Math.min(levels.length - 1, Math.floor((bar * levels.length) / count));
    const to = Math.max(from + 1, Math.floor(((bar + 1) * levels.length) / count));
    let loudest = 0;
    for (let i = from; i < Math.min(to, levels.length); i += 1) loudest = Math.max(loudest, levels[i] ?? 0);
    maxima[bar] = Number.isFinite(loudest) ? loudest : 0;
  }
  return normalise(maxima);
}

/** The same for decoded samples (one channel, -1 to 1): the level of a window is `levelOfRms`, so a recorded and a decoded note look alike. */
export function peaksFromSamples(samples: ArrayLike<number>, sampleRate: number, count: number = WAVEFORM_PEAKS): Uint8Array | null {
  const window = Math.max(1, Math.round((sampleRate * WINDOW_MS) / 1000));
  const levels: number[] = [];
  for (let start = 0; start < samples.length; start += window) {
    const end = Math.min(samples.length, start + window);
    let sum = 0;
    for (let i = start; i < end; i += 1) {
      const value = samples[i] ?? 0;
      sum += value * value;
    }
    levels.push(levelOfRms(Math.sqrt(sum / (end - start))));
  }
  return peaksFromLevels(levels, count);
}

function normalise(maxima: Float64Array): Uint8Array | null {
  let loudest = 0;
  for (const value of maxima) loudest = Math.max(loudest, value);
  if (loudest <= 0) return null;
  return Uint8Array.from(maxima, (value) => Math.round((value / loudest) * 255));
}

/** The decoder of this web view, or `null` where there is none (jsdom, an unusual build). `OfflineAudioContext` needs no audio device. */
interface Decoder {
  decodeAudioData(data: ArrayBuffer): Promise<AudioBuffer>;
}

function makeDecoder(): Decoder | null {
  const Offline = typeof OfflineAudioContext === "undefined" ? undefined : OfflineAudioContext;
  return Offline ? new Offline(1, 1, DECODE_SAMPLE_RATE) : null;
}

/**
 * Decodes a voice note's file and gives its peaks. Rejects when the web view cannot decode the file (an unsupported container, a damaged
 * file): the caller stores the empty "failed" marker so the file is never tried again. Nothing here reads a device or a label.
 */
export async function decodePeaks(bytes: ArrayBuffer): Promise<Uint8Array> {
  const decoder = makeDecoder();
  if (!decoder) throw new Error("no decoder");
  // `decodeAudioData` detaches the buffer it is given; the caller's copy is not needed afterwards.
  const decoded = await decoder.decodeAudioData(bytes);
  // Every channel counts: the loudest of them in each sample makes the window level, so a note panned to one side still has a shape.
  const length = decoded.length;
  const mixed = new Float32Array(length);
  for (let channel = 0; channel < decoded.numberOfChannels; channel += 1) {
    const data = decoded.getChannelData(channel);
    for (let i = 0; i < length; i += 1) {
      const value = data[i] ?? 0;
      if (Math.abs(value) > Math.abs(mixed[i] ?? 0)) mixed[i] = value;
    }
  }
  const peaks = peaksFromSamples(mixed, decoded.sampleRate);
  if (!peaks) throw new Error("silent or empty");
  return peaks;
}
