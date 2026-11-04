import { openMicrophone } from "virtual:kinige-mic";

// One recording on top of the web view's MediaRecorder (slice 7a, ADR-0015). No React here: the hook `useVoiceRecorder` drives it and the
// tests drive it with a fake `MediaDevices`. The microphone is asked for in `startSession`, which the first click on the microphone button
// calls, never at startup (PRD §15). Nothing here reads a device label: it is personal data and never reaches the console, a string or Rust.

/** A recording is sent by itself at this length (decision 3). */
export const MAX_RECORDING_MS = 30 * 60 * 1000;
/** ... or at this many bytes, whichever comes first: `audioBitsPerSecond` is only a hint, and the IPC body is capped at 64 MiB. */
export const MAX_RECORDING_BYTES = 48 * 1024 * 1024;
/** The timer turns to its warning colour from here. */
export const WARN_FROM_MS = 29 * 60 * 1000;
/** Shorter than this is not saved ("Too short to save"). */
export const MIN_RECORDING_MS = 1000;
/** Cancelling a recording shorter than this drops it at once; a longer one asks first. */
export const CONFIRM_AFTER_MS = 5000;
/** The bars of the level meter. */
export const LEVEL_BARS = 24;

const AUDIO_BITS_PER_SECOND = 64_000;
const TIMESLICE_MS = 1000;
const LEVEL_PERIOD_MS = 100;
/** The first supported of these; the container comes out of the first bytes in Rust, never out of this choice. */
const RECORDER_TYPES = ["audio/mp4;codecs=mp4a.40.2", "audio/mp4", "audio/webm;codecs=opus"];

/** Why a recording did not start, as the person is told. */
export type MicError = "denied" | "no-device" | "busy";

/** `getUserMedia` rejects with a DOMException; its name tells the cause. A missing `mediaDevices` is the same as no microphone. */
export function micErrorOf(reason: unknown): MicError {
  // Read the name structurally: a DOMException from another realm (an iframe, a test environment) is not `instanceof Error`.
  const name = typeof reason === "object" && reason !== null && "name" in reason ? String(reason.name) : "";
  if (name === "NotAllowedError" || name === "SecurityError") return "denied";
  if (name === "NotFoundError" || name === "OverconstrainedError") return "no-device";
  return "busy";
}

/** The recording a finished session hands over. */
export interface Recording {
  bytes: Uint8Array;
  /** The recorder's own clock, in milliseconds. */
  durationMs: number;
}

export interface SessionHooks {
  /** The microphone went away mid-recording (the track ended): what was recorded is kept. */
  onDeviceLost: () => void;
  /** The recording reached the byte limit. */
  onByteLimit: () => void;
}

/** A running recording. All of it ends with `finish` (keep the bytes) or `abort` (drop them); both release the microphone. */
export class RecorderSession {
  private readonly chunks: Blob[] = [];
  private byteCount = 0;
  private readonly startedAt: number;
  private endedAt: number | null = null;
  private levels: number[] = new Array<number>(LEVEL_BARS).fill(0);
  private context: AudioContext | null = null;
  private analyser: AnalyserNode | null = null;
  private meter: ReturnType<typeof setInterval> | null = null;
  private stopped: Promise<void> | null = null;
  private released = false;
  private limitReported = false;

  private constructor(
    private readonly stream: MediaStream,
    private readonly recorder: MediaRecorder,
    hooks: SessionHooks,
  ) {
    this.startedAt = performance.now();
    recorder.addEventListener("dataavailable", (event) => {
      if (event.data.size === 0) return;
      this.chunks.push(event.data);
      this.byteCount += event.data.size;
      if (this.byteCount >= MAX_RECORDING_BYTES && !this.limitReported) {
        this.limitReported = true;
        hooks.onByteLimit();
      }
    });
    for (const track of stream.getAudioTracks()) {
      track.addEventListener("ended", () => {
        if (this.released || this.endedAt !== null) return;
        // Stop the recorder so its last chunk arrives; the hook then offers Send and Discard.
        this.endedAt = performance.now();
        this.stopRecorder();
        hooks.onDeviceLost();
      });
    }
    this.startMeter();
  }

  /** Asks for the microphone and starts recording. Rejects with the `getUserMedia` error (see [`micErrorOf`]). */
  static async start(hooks: SessionHooks): Promise<RecorderSession> {
    if (typeof MediaRecorder === "undefined") throw new DOMException("no MediaRecorder", "NotFoundError");
    const stream = await openMicrophone();
    try {
      const mimeType = RECORDER_TYPES.find((type) => MediaRecorder.isTypeSupported(type));
      const recorder = new MediaRecorder(stream, {
        ...(mimeType ? { mimeType } : {}),
        audioBitsPerSecond: AUDIO_BITS_PER_SECOND,
      });
      const session = new RecorderSession(stream, recorder, hooks);
      recorder.start(TIMESLICE_MS);
      return session;
    } catch (reason) {
      for (const track of stream.getTracks()) track.stop();
      throw reason;
    }
  }

  /**
   * When the recording is sent by itself: 30 minutes, or what the stream asks for. Only the screens runners' fake microphone asks for less:
   * on a macOS runner every accessibility call hangs for as long as a recording runs, so the scenario's recording must end by itself.
   */
  limitMs(): number {
    const asked = "maxRecordingMs" in this.stream ? Number(this.stream.maxRecordingMs) : NaN;
    return Number.isFinite(asked) && asked > 0 ? Math.min(asked, MAX_RECORDING_MS) : MAX_RECORDING_MS;
  }

  /** Milliseconds recorded so far (frozen when the device was lost). */
  elapsedMs(): number {
    return Math.max(0, Math.round((this.endedAt ?? performance.now()) - this.startedAt));
  }

  bytes(): number {
    return this.byteCount;
  }

  /** The last [`LEVEL_BARS`] levels, 0 to 1, oldest first. */
  levelBars(): readonly number[] {
    return this.levels;
  }

  /**
   * Stops, releases the microphone and gives the recording. The clock is read before the recorder stops, and never past 30 minutes: the
   * auto-send tick can come late (a hidden window, a swapped-out VM), and the store refuses what is over 30 minutes and 5 seconds.
   */
  async finish(): Promise<Recording> {
    const durationMs = Math.min(this.elapsedMs(), MAX_RECORDING_MS);
    await this.stopRecorder();
    this.release();
    const blob = new Blob(this.chunks);
    return { bytes: new Uint8Array(await blob.arrayBuffer()), durationMs };
  }

  /** Drops the recording and releases the microphone. */
  abort(): void {
    this.chunks.length = 0;
    this.byteCount = 0;
    this.stopRecorder().catch(() => undefined);
    this.release();
  }

  /** Resolves when the recorder has delivered its last chunk. Safe to call twice. */
  private stopRecorder(): Promise<void> {
    if (this.stopped) return this.stopped;
    this.stopped = new Promise<void>((resolve) => {
      if (this.recorder.state === "inactive") {
        resolve();
        return;
      }
      this.recorder.addEventListener("stop", () => resolve(), { once: true });
      try {
        this.recorder.stop();
      } catch {
        resolve();
      }
    });
    return this.stopped;
  }

  /** Every path ends here: the tracks stop (the system's microphone indicator goes off) and the audio context closes. */
  private release(): void {
    if (this.released) return;
    this.released = true;
    if (this.meter !== null) clearInterval(this.meter);
    this.meter = null;
    for (const track of this.stream.getTracks()) track.stop();
    this.context?.close().catch(() => undefined);
    this.context = null;
    this.analyser = null;
  }

  private startMeter(): void {
    if (typeof AudioContext === "undefined") return;
    // The screens runners' fake microphone asks for no meter: a second AudioContext on a VM without an audio device is part of what made the
    // window tree unreadable while recording (see `scripts/screens/fake-mic.ts`).
    if ("withoutMeter" in this.stream) return;
    try {
      this.context = new AudioContext();
      this.analyser = this.context.createAnalyser();
      this.analyser.fftSize = 256;
      this.context.createMediaStreamSource(this.stream).connect(this.analyser);
    } catch {
      this.context = null;
      this.analyser = null;
      return;
    }
    const samples = new Uint8Array(this.analyser.fftSize);
    this.meter = setInterval(() => {
      if (!this.analyser) return;
      this.analyser.getByteTimeDomainData(samples);
      let sum = 0;
      for (const sample of samples) {
        const centered = (sample - 128) / 128;
        sum += centered * centered;
      }
      const rms = Math.sqrt(sum / samples.length);
      // Speech sits low in RMS: a square root spreads it over the bar height.
      this.levels = [...this.levels.slice(1), Math.min(1, Math.sqrt(rms) * 1.4)];
    }, LEVEL_PERIOD_MS);
  }
}
