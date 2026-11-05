// The one-hour microphone probe of slice 7a (spec, "Probe first"). `web/vite.config.ts` points `virtual:kinige-probe` here only when
// the build is started with KINIGE_PROBE=1 (the `7a-probe` screens scenario). It runs inside the real webview of the built app
// (tauri://localhost on WKWebView, http://tauri.localhost on WebView2), records five seconds from the fake microphone and reports the
// facts as a plain-text entry in the Inbox of the synthetic profile; the scenario reads that entry from the database into
// `formats.txt`. The report never holds a device label or a device name.
//
// It runs twice: once at load (no user gesture) and once on the first trusted click or key (a user gesture), because an
// AudioContext may stay suspended without one.
import { openMicrophone } from "./fake-mic";

type Report = (text: string) => Promise<unknown>;

const RECORD_MS = 5000;
const GUARD_MS = 25000;
const RECORDER_TYPES = ["audio/mp4;codecs=mp4a.40.2", "audio/mp4", "audio/webm;codecs=opus"];
const PLAYBACK_TYPES = [...RECORDER_TYPES, "audio/ogg; codecs=opus"];

export function startProbe(report: Report): void {
  void run("without a user gesture", report);
  const onGesture = () => {
    window.removeEventListener("click", onGesture, true);
    window.removeEventListener("keydown", onGesture, true);
    void run("on the first user gesture", report);
  };
  window.addEventListener("click", onGesture, true);
  window.addEventListener("keydown", onGesture, true);
}

async function run(title: string, report: Report): Promise<void> {
  const lines: string[] = [`KINIGE PROBE 7a, ${title}`];
  const add = (key: string, value: unknown) => lines.push(`${key}: ${String(value)}`);
  const guard = new Promise<void>((resolve) => window.setTimeout(resolve, GUARD_MS)).then(() => add("guard", "timed out"));
  try {
    await Promise.race([facts(add), guard]);
  } catch (reason) {
    add("probe error", errorName(reason));
  }
  await report(lines.join("\n")).catch(() => undefined);
}

async function facts(add: (key: string, value: unknown) => void): Promise<void> {
  add("origin", window.location.origin);
  add("isSecureContext", window.isSecureContext);
  add("navigator.mediaDevices", Boolean(navigator.mediaDevices));
  add("MediaRecorder", typeof MediaRecorder);
  add("AudioContext", typeof AudioContext);
  add("userActivation.isActive", navigator.userActivation?.isActive);
  add("userActivation.hasBeenActive", navigator.userActivation?.hasBeenActive);
  add("userAgent", navigator.userAgent);
  try {
    const kinds = (await navigator.mediaDevices?.enumerateDevices())?.map((device) => device.kind) ?? [];
    add("audioinput devices (count only)", kinds.filter((kind) => kind === "audioinput").length);
  } catch (reason) {
    add("enumerateDevices error", errorName(reason));
  }
  for (const type of RECORDER_TYPES) {
    add(`MediaRecorder.isTypeSupported(${type})`, typeof MediaRecorder === "undefined" ? "n/a" : MediaRecorder.isTypeSupported(type));
  }
  const audio = document.createElement("audio");
  for (const type of PLAYBACK_TYPES) add(`canPlayType(${type})`, audio.canPlayType(type) || "(empty)");

  if (typeof MediaRecorder === "undefined") return;
  const mimeType = RECORDER_TYPES.find((type) => MediaRecorder.isTypeSupported(type));
  add("recorder type chosen", mimeType ?? "none");
  if (!mimeType) return;
  await recordAndDecode(mimeType, add);
}

async function recordAndDecode(mimeType: string, add: (key: string, value: unknown) => void): Promise<void> {
  const stream = await openMicrophone();
  add("fake source tracks", stream.getAudioTracks().length);
  try {
    const recorder = new MediaRecorder(stream, { mimeType, audioBitsPerSecond: 64000 });
    const chunks: Blob[] = [];
    recorder.addEventListener("dataavailable", (event) => {
      if (event.data.size > 0) chunks.push(event.data);
    });
    const stopped = new Promise<void>((resolve) => recorder.addEventListener("stop", () => resolve(), { once: true }));
    const started = performance.now();
    recorder.start(1000);
    await new Promise((resolve) => window.setTimeout(resolve, RECORD_MS));
    recorder.stop();
    await stopped;
    const seconds = (performance.now() - started) / 1000;
    const blob = new Blob(chunks);
    add("recorder.mimeType", recorder.mimeType || "(empty)");
    add("chunks", chunks.length);
    add("bytes", blob.size);
    add("seconds (own clock)", seconds.toFixed(2));
    add("observed bytes per second", (blob.size / seconds).toFixed(0));
    const buffer = await blob.arrayBuffer();
    add("first 12 bytes (hex)", hex(new Uint8Array(buffer.slice(0, 12))));
    await decode(buffer, add);
  } finally {
    for (const track of stream.getTracks()) track.stop();
  }
}

async function decode(buffer: ArrayBuffer, add: (key: string, value: unknown) => void): Promise<void> {
  const context = new AudioContext();
  try {
    add("decode AudioContext.state", context.state);
    const decoded = await context.decodeAudioData(buffer.slice(0));
    let peak = 0;
    const samples = decoded.getChannelData(0);
    for (let index = 0; index < samples.length; index += 1) {
      peak = Math.max(peak, Math.abs(samples[index] ?? 0));
    }
    add("decoded duration (s)", decoded.duration.toFixed(2));
    add("decoded channels", decoded.numberOfChannels);
    add("decoded sample rate", decoded.sampleRate);
    add("decoded peak amplitude (0 = silence)", peak.toFixed(3));
  } catch (reason) {
    add("decodeAudioData error", errorName(reason));
  } finally {
    void context.close().catch(() => undefined);
  }
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join(" ");
}

function errorName(reason: unknown): string {
  return reason instanceof Error ? reason.name : "unknown";
}
