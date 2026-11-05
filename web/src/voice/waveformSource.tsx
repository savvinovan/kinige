import { createContext, useContext, useEffect, useMemo, useState, type RefObject } from "react";
import type { AttachmentView } from "../api/generated/AttachmentView";
import { decodePeaks } from "./waveform";

// Where a voice note's waveform comes from when the row has none (slice 7a, ADR-0015): the player decodes the file once, when it first
// shows on screen, and saves the peaks through the main window's IPC. One decode at a time; a file over the cap, or one this web view
// cannot decode, gets the empty "failed" marker and is never tried again.

/** The decoder is not asked to read more than this many bytes (the same cap as `kinige_store::MAX_DECODE_BYTES`) ... */
export const MAX_DECODE_BYTES = 16 * 1024 * 1024;
/** ... or, for a note with no length in its row, more than this many bytes (a long low-bitrate file could take hundreds of MB of samples) ... */
export const MAX_DECODE_BYTES_UNKNOWN_LENGTH = 8 * 1024 * 1024;
/** ... or a note longer than this: its samples would take too much memory. */
export const MAX_DECODE_MS = 20 * 60 * 1000;
/** One decode (the read and the decoding) gets this long; then the queue moves on and nothing is written. */
export const DECODE_TIMEOUT_MS = 15_000;

/** What the player needs from the app: the file's bytes and a place for the peaks. The default is none (stories, tests): placeholder bars. */
export interface WaveformSource {
  read: (attachmentId: string) => Promise<ArrayBuffer>;
  save: (attachmentId: string, peaks: Uint8Array) => Promise<void>;
}

export const WaveformSourceContext = createContext<WaveformSource | null>(null);

/** Peaks the window already knows: decoded this session, or told by the feed. The player on screen again after a scroll finds them here. */
const known = new Map<string, Uint8Array>();
const pending = new Map<string, Promise<Uint8Array | null>>();
/** Notes whose decode timed out this session. */
const timedOut = new Set<string>();
let tail: Promise<unknown> = Promise.resolve();

/** Tests only: forget what the session knew. */
export function resetWaveformSession(): void {
  known.clear();
  pending.clear();
  timedOut.clear();
  tail = Promise.resolve();
}

/**
 * Peaks for a note that has none: the file is read, decoded and saved, one note at a time. Resolves to the peaks (empty for "cannot be
 * decoded"), or `null` when nothing could be read now (a transient error: the note is tried again the next time it shows).
 */
export function ensurePeaks(source: WaveformSource, attachment: AttachmentView): Promise<Uint8Array | null> {
  const id = attachment.id;
  const have = known.get(id);
  if (have) return Promise.resolve(have);
  const running = pending.get(id);
  if (running) return running;
  // A note whose decode ran out of time is not tried again until the app restarts.
  if (timedOut.has(id)) return Promise.resolve(null);
  const job = tail.then(() => decodeWithin(source, attachment));
  tail = job.catch(() => undefined);
  const guarded = job.finally(() => pending.delete(id));
  pending.set(id, guarded);
  return guarded;
}

/** One note's decode with a time limit: on a timeout nothing is written, the queue moves on, and the note is retried next session. */
async function decodeWithin(source: WaveformSource, attachment: AttachmentView): Promise<Uint8Array | null> {
  const run = { stale: false };
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<"timeout">((resolve) => {
    timer = setTimeout(() => resolve("timeout"), DECODE_TIMEOUT_MS);
  });
  try {
    const result = await Promise.race([decodeAndSave(source, attachment, run), timeout]);
    if (result === "timeout") {
      // The decode may still finish later: it must not write anything then.
      run.stale = true;
      timedOut.add(attachment.id);
      return null;
    }
    return result;
  } finally {
    clearTimeout(timer);
  }
}

async function decodeAndSave(source: WaveformSource, attachment: AttachmentView, run: { stale: boolean }): Promise<Uint8Array | null> {
  const id = attachment.id;
  // With no length in the row (a Telegram voice) the file is not known to be short: a lower cap on the bytes keeps the samples small.
  const byteCap = attachment.durationMs === null ? MAX_DECODE_BYTES_UNKNOWN_LENGTH : MAX_DECODE_BYTES;
  let peaks: Uint8Array;
  if (attachment.byteSize > byteCap || (attachment.durationMs ?? 0) > MAX_DECODE_MS) {
    peaks = new Uint8Array(0);
  } else {
    let bytes: ArrayBuffer;
    try {
      bytes = await source.read(id);
    } catch {
      return null;
    }
    try {
      peaks = await decodePeaks(bytes);
    } catch {
      peaks = new Uint8Array(0);
    }
  }
  if (run.stale) return null;
  known.set(id, peaks);
  // The peaks are shown either way; a save that fails only means the next start decodes again.
  await source.save(id, peaks).catch(() => undefined);
  return peaks;
}

/**
 * The peaks a player draws, and the work of getting them: nothing happens until the player is on screen (an `IntersectionObserver` on
 * `host`), and a note whose row already has peaks never decodes. Without a source in the tree (stories, tests) the peaks stay `null`.
 */
export function useWaveform(attachment: AttachmentView, host: RefObject<HTMLElement | null>, wanted: boolean): Uint8Array | null {
  const source = useContext(WaveformSourceContext);
  const [decoded, setDecoded] = useState<Uint8Array | null>(() => known.get(attachment.id) ?? null);
  const fromRow = useMemo(() => (attachment.waveform ? Uint8Array.from(attachment.waveform) : null), [attachment.waveform]);
  const needs = wanted && fromRow === null && decoded === null && source !== null;

  useEffect(() => {
    const element = host.current;
    if (!needs || !source || !element || typeof IntersectionObserver === "undefined") return;
    let cancelled = false;
    const observer = new IntersectionObserver((entries) => {
      if (!entries.some((entry) => entry.isIntersecting)) return;
      observer.disconnect();
      void ensurePeaks(source, attachment).then((peaks) => {
        if (!cancelled && peaks) setDecoded(peaks);
      });
    });
    observer.observe(element);
    return () => {
      cancelled = true;
      observer.disconnect();
    };
  }, [needs, source, attachment, host]);

  return fromRow ?? decoded;
}
