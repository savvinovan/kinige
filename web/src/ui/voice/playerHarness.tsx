import { vi } from "vitest";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { act, fireEvent, render } from "../../test/render";
import { demoPeaks, demoVoice } from "../attachments/demo";
import { MediaSourceContext } from "../attachments/mediaUrl";
import { resetPlayerSession } from "./playerSession";
import { resetPlayableCache, VoicePlayer } from "./VoicePlayer";

// What the tests of the voice player share (slice 7a): jsdom has no media engine and no animation frames the test can step, so the media
// element only remembers what it was asked, and the animation frames the player asks for are collected and run by hand.

const SOURCE = (variant: string, id: string) => `kinige-media://localhost/${variant}/${id}`;

export const harness = {
  canPlay: "maybe",
  played: [] as HTMLMediaElement[],
  paused: [] as HTMLMediaElement[],
  /** The animation frames the player asked for. */
  frames: new Map<number, FrameRequestCallback>(),
  nextFrame: 1,
};

/** Call in `beforeEach`; undo with `vi.restoreAllMocks()` and `vi.unstubAllGlobals()` in `afterEach`. */
export function installPlayerMocks(): void {
  resetPlayerSession();
  resetPlayableCache();
  harness.canPlay = "maybe";
  harness.played.length = 0;
  harness.paused.length = 0;
  harness.frames = new Map();
  harness.nextFrame = 1;
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    const id = harness.nextFrame++;
    harness.frames.set(id, callback);
    return id;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => harness.frames.delete(id));
  vi.spyOn(HTMLMediaElement.prototype, "canPlayType").mockImplementation(() => harness.canPlay as CanPlayTypeResult);
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(function (this: HTMLMediaElement) {
    harness.played.push(this);
    Object.defineProperty(this, "paused", { value: false, configurable: true });
    this.dispatchEvent(new Event("play"));
    return Promise.resolve();
  });
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(function (this: HTMLMediaElement) {
    harness.paused.push(this);
    Object.defineProperty(this, "paused", { value: true, configurable: true });
    this.dispatchEvent(new Event("pause"));
  });
}

export function show(attachment: AttachmentView, onOpenFile = vi.fn(), language: "en" | "ru" = "en") {
  const result = render(
    <MediaSourceContext.Provider value={SOURCE}>
      <VoicePlayer attachment={attachment} onOpenFile={onOpenFile} />
    </MediaSourceContext.Provider>,
    { language },
  );
  // Null for the tile and the "Can't play here" line, which have no media element.
  const audio = result.container.querySelector("audio") as HTMLAudioElement;
  const wave = result.container.querySelector(".voice-player__wave") as HTMLElement;
  const canvas = result.container.querySelector("canvas") as HTMLCanvasElement | null;
  return { ...result, audio, wave, canvas, onOpenFile };
}

/** The media element reports a position. */
export function tick(element: HTMLMediaElement, seconds: number) {
  Object.defineProperty(element, "currentTime", { value: seconds, configurable: true, writable: true });
  fireEvent.timeUpdate(element);
}

/** Runs the one pending animation frame, with the media element at `seconds`. */
export function frame(element: HTMLMediaElement, seconds: number) {
  Object.defineProperty(element, "currentTime", { value: seconds, configurable: true, writable: true });
  const [id, callback] = [...harness.frames.entries()][0] ?? [];
  if (id === undefined || !callback) throw new Error("no animation frame is pending");
  harness.frames.delete(id);
  act(() => callback(performance.now()));
}

/** The played share of the waveform as the player last painted it (0 to 1). */
export const progress = (wave: HTMLElement) => Number(wave.dataset.progress);

export const NOTE = () => demoVoice("v1", 40_000, { waveform: demoPeaks(1), byteSize: 21_196 });
