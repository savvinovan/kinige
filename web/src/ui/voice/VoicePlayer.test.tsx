import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { rect } from "../../test/rect";
import { act, fireEvent, render, screen } from "../../test/render";
import { demoPeaks, demoVoice } from "../attachments/demo";
import { MediaSourceContext } from "../attachments/mediaUrl";
import { resetPlayerSession } from "./playerSession";
import { resetPlayableCache, showsWaveform, VoicePlayer } from "./VoicePlayer";

const SOURCE = (variant: string, id: string) => `kinige-media://localhost/${variant}/${id}`;

let canPlay = "maybe";
const played: HTMLMediaElement[] = [];
const paused: HTMLMediaElement[] = [];
/** The animation frames the player asked for; a test runs them by hand. */
let frames = new Map<number, FrameRequestCallback>();
let nextFrame = 1;

beforeEach(() => {
  resetPlayerSession();
  resetPlayableCache();
  canPlay = "maybe";
  played.length = 0;
  paused.length = 0;
  frames = new Map();
  nextFrame = 1;
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    const id = nextFrame++;
    frames.set(id, callback);
    return id;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
  // jsdom has no media engine: the element only remembers what it was asked.
  vi.spyOn(HTMLMediaElement.prototype, "canPlayType").mockImplementation(() => canPlay as CanPlayTypeResult);
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(function (this: HTMLMediaElement) {
    played.push(this);
    Object.defineProperty(this, "paused", { value: false, configurable: true });
    this.dispatchEvent(new Event("play"));
    return Promise.resolve();
  });
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(function (this: HTMLMediaElement) {
    paused.push(this);
    Object.defineProperty(this, "paused", { value: true, configurable: true });
    this.dispatchEvent(new Event("pause"));
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(document, "hidden");
});

function show(attachment: AttachmentView, onOpenFile = vi.fn(), language: "en" | "ru" = "en") {
  const result = render(
    <MediaSourceContext.Provider value={SOURCE}>
      <VoicePlayer attachment={attachment} onOpenFile={onOpenFile} />
    </MediaSourceContext.Provider>,
    { language },
  );
  // Null for the tile and the "Can't play here" line, which have no media element.
  const audio = result.container.querySelector("audio") as HTMLAudioElement;
  const wave = result.container.querySelector(".voice-player__wave") as HTMLElement;
  return { ...result, audio, wave, onOpenFile };
}

/** The media element reports a position. */
function tick(element: HTMLMediaElement, seconds: number) {
  Object.defineProperty(element, "currentTime", { value: seconds, configurable: true, writable: true });
  fireEvent.timeUpdate(element);
}

/** Runs the one pending animation frame, with the media element at `seconds`. */
function frame(element: HTMLMediaElement, seconds: number) {
  Object.defineProperty(element, "currentTime", { value: seconds, configurable: true, writable: true });
  const [id, callback] = [...frames.entries()][0] ?? [];
  if (id === undefined || !callback) throw new Error("no animation frame is pending");
  frames.delete(id);
  act(() => callback(performance.now()));
}

const progress = (wave: HTMLElement) => wave.style.getPropertyValue("--voice-progress");
const NOTE = () => demoVoice("v1", 40_000, { waveform: demoPeaks(1), byteSize: 21_196 });

describe("VoicePlayer", () => {
  it("is labelled for the screen reader and opens a chat without asking for any audio", () => {
    const { audio } = show(NOTE());
    expect(screen.getByRole("group", { name: "Voice note, 40 seconds" })).toBeInTheDocument();
    expect(audio.getAttribute("src")).toBeNull();
    expect(audio.preload).toBe("none");
    expect(played).toHaveLength(0);
    // The total and the size at rest; no speed chip until the note has been started.
    expect(screen.getByText(/^0:40, 20\.7/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Playback speed/ })).toBeNull();
    expect(frames.size).toBe(0);
  });

  it("is labelled in Russian with the right plural and the size in Russian units", () => {
    show(NOTE(), vi.fn(), "ru");
    expect(screen.getByRole("group", { name: "Голосовое сообщение, 40 секунд" })).toBeInTheDocument();
    expect(screen.getByText(/^0:40, 20,7 /)).toBeInTheDocument();
  });

  it("draws one bar per peak in two layers, the played layer cut at the position", () => {
    const { wave } = show(NOTE());
    const layers = wave.querySelectorAll(".voice-player__bars");
    expect(layers).toHaveLength(2);
    expect(layers[1]).toHaveClass("voice-player__bars--played");
    for (const layer of layers) expect(layer.querySelectorAll(".voice-player__bar")).toHaveLength(64);
    const first = layers[0]?.querySelector<HTMLElement>(".voice-player__bar");
    expect(first?.style.getPropertyValue("--peak")).toBe(((demoPeaks(1)[0] ?? 0) / 255).toFixed(3));
    expect(layers[0]).toHaveAttribute("aria-hidden", "true");
    expect(progress(wave)).toBe("0.00%");
  });

  it("draws neutral placeholder bars of the same layout while the note has no peaks", () => {
    const { wave } = show(demoVoice("v1", 40_000, { byteSize: 21_196 }));
    const bars = [...wave.querySelectorAll<HTMLElement>(".voice-player__bar")];
    expect(bars).toHaveLength(128);
    const heights = new Set(bars.map((bar) => bar.style.getPropertyValue("--peak")));
    expect(heights.size).toBeGreaterThan(8);
    for (const height of heights) expect(Number(height)).toBeLessThan(0.6);
  });

  it("sets the source only on the first Play, then shows the elapsed time over the same line", () => {
    const { audio } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(audio.getAttribute("src")).toBe("kinige-media://localhost/audio/v1");
    expect(played).toEqual([audio]);
    expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
    tick(audio, 3);
    expect(screen.getByText(/^0:03, 20\.7/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(paused).toEqual([audio]);
    expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
    // The paused position stays on the line and in the fill.
    expect(screen.getByText(/^0:03, /)).toBeInTheDocument();
    // A second Play does not change the source.
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(audio.getAttribute("src")).toBe("kinige-media://localhost/audio/v1");
  });

  it("an hour or more reads as h:mm:ss", () => {
    const { audio } = show(demoVoice("long", 3_723_000, { waveform: demoPeaks(2), byteSize: 29_000_000 }));
    expect(screen.getByText(/^1:02:03, /)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    tick(audio, 3_661);
    expect(screen.getByText(/^1:01:01, /)).toBeInTheDocument();
  });

  it("the speed chip shows after the first Play and cycles 1× → 1.5× → 2× → 1× for the whole session", () => {
    const { audio, unmount } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    const chip = () => screen.getByRole("button", { name: /^Playback speed/ });
    expect(chip()).toHaveTextContent("1×");
    fireEvent.click(chip());
    expect(chip()).toHaveTextContent("1.5×");
    fireEvent.click(chip());
    expect(chip()).toHaveTextContent("2×");
    expect(audio.playbackRate).toBe(2);
    fireEvent.click(chip());
    expect(chip()).toHaveTextContent("1×");
    fireEvent.click(chip());
    unmount();
    const next = show(demoVoice("v2", 10_000, { waveform: demoPeaks(3) }));
    fireEvent.click(next.container.querySelector(".voice-player__play") as HTMLElement);
    expect(screen.getByRole("button", { name: /^Playback speed/ })).toHaveTextContent("1.5×");
  });

  it("a playing note follows the speed another note's chip sets", () => {
    const first = show(NOTE());
    const second = show(demoVoice("b", 20_000, { waveform: demoPeaks(4) }));
    fireEvent.click(screen.getAllByRole("button", { name: "Play" })[0] as HTMLElement);
    fireEvent.click(screen.getAllByRole("button", { name: "Play" })[0] as HTMLElement);
    expect(second.audio.src).not.toBe("");
    fireEvent.click(screen.getAllByRole("button", { name: /^Playback speed/ })[0] as HTMLElement);
    expect(first.audio.playbackRate).toBe(1.5);
  });

  it("starting another voice note pauses the first one", () => {
    const first = show(demoVoice("a", 10_000, { waveform: demoPeaks(1) }));
    const second = show(demoVoice("b", 20_000, { waveform: demoPeaks(2) }));
    const [playA, playB] = screen.getAllByRole("button", { name: "Play" });
    fireEvent.click(playA as HTMLElement);
    expect(played).toEqual([first.audio]);
    fireEvent.click(playB as HTMLElement);
    expect(paused).toContain(first.audio);
    expect(played).toEqual([first.audio, second.audio]);
    expect(screen.getAllByRole("button", { name: "Play" })).toHaveLength(1);
  });
});

describe("the fill follows an animation frame while playing", () => {
  it("runs a frame loop while playing, moves the fill inside a second and re-renders only when the whole second changes", () => {
    const { audio, wave } = show(NOTE());
    expect(frames.size).toBe(0);
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(frames.size).toBe(1);
    frame(audio, 2.0);
    expect(progress(wave)).toBe("5.00%");
    expect(screen.getByText(/^0:02, /)).toBeInTheDocument();
    // The media element's own `timeupdate` has not fired (it does about four times a second); the frames move the fill on their own.
    const before = progress(wave);
    frame(audio, 2.04);
    expect(progress(wave)).toBe("5.10%");
    expect(progress(wave)).not.toBe(before);
    frame(audio, 2.5);
    expect(progress(wave)).toBe("6.25%");
    expect(screen.getByText(/^0:02, /)).toBeInTheDocument();
    // Every frame asks for the next one.
    expect(frames.size).toBe(1);
    frame(audio, 3.01);
    expect(screen.getByText(/^0:03, /)).toBeInTheDocument();
  });

  it("the fill can stop inside a bar (a share of a bar, not only whole bars)", () => {
    const { audio, wave } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    // A bar is 1/64 of the width = 1.5625 %: 3.30 % is a bar and a half.
    frame(audio, 1.32);
    const value = Number.parseFloat(progress(wave));
    expect(value).toBeCloseTo(3.3, 5);
    expect((value / (100 / 64)) % 1).not.toBe(0);
  });

  it("stops the loop when the note is paused", () => {
    const { audio, wave } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    frame(audio, 4);
    expect(frames.size).toBe(1);
    Object.defineProperty(audio, "currentTime", { value: 4.2, configurable: true, writable: true });
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(frames.size).toBe(0);
    // The paused position is painted once, from the element.
    expect(progress(wave)).toBe("10.50%");
  });

  it("stops the loop when the note ends and rewinds the fill", () => {
    const { audio, wave } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    frame(audio, 39.9);
    fireEvent.ended(audio);
    expect(frames.size).toBe(0);
    expect(progress(wave)).toBe("0.00%");
    expect(screen.getByText(/^0:40, /)).toBeInTheDocument();
  });

  it("stops the loop when the player goes away", () => {
    const { unmount } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(frames.size).toBe(1);
    unmount();
    expect(frames.size).toBe(0);
  });

  it("stops the loop while the page is hidden and starts it again when it shows", () => {
    show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(frames.size).toBe(1);
    Object.defineProperty(document, "hidden", { value: true, configurable: true });
    document.dispatchEvent(new Event("visibilitychange"));
    expect(frames.size).toBe(0);
    Object.defineProperty(document, "hidden", { value: false, configurable: true });
    document.dispatchEvent(new Event("visibilitychange"));
    expect(frames.size).toBe(1);
  });

  it("a note that is only at rest never schedules a frame", () => {
    show(NOTE());
    show(demoVoice("b", 9_000));
    expect(frames.size).toBe(0);
  });
});

describe("seeking", () => {
  it("the waveform is a slider with a value text and an accessible name", () => {
    const { wave } = show(NOTE());
    const slider = screen.getByRole("slider", { name: "Position" });
    expect(slider).toBe(wave);
    expect(slider).toHaveAttribute("aria-valuemin", "0");
    expect(slider).toHaveAttribute("aria-valuemax", "40000");
    expect(slider).toHaveAttribute("aria-valuenow", "0");
    expect(slider).toHaveAttribute("aria-valuetext", "0:00 of 0:40");
    expect(slider).toHaveAttribute("tabindex", "0");
  });

  it("the arrow keys move the position by five seconds, and a seek before Play waits for the file", () => {
    const { audio } = show(NOTE());
    const seek = screen.getByRole("slider", { name: "Position" });
    fireEvent.keyDown(seek, { key: "ArrowRight" });
    expect(seek).toHaveAttribute("aria-valuenow", "5000");
    expect(seek).toHaveAttribute("aria-valuetext", "0:05 of 0:40");
    expect(screen.getByText(/^0:05, /)).toBeInTheDocument();
    expect(audio.getAttribute("src")).toBeNull();
    fireEvent.keyDown(seek, { key: "ArrowRight" });
    fireEvent.keyDown(seek, { key: "ArrowLeft" });
    expect(seek).toHaveAttribute("aria-valuenow", "5000");
    fireEvent.keyDown(seek, { key: "ArrowLeft" });
    fireEvent.keyDown(seek, { key: "ArrowLeft" });
    expect(seek).toHaveAttribute("aria-valuenow", "0");
    // At the end it stops at the total; Home and End jump.
    for (let i = 0; i < 12; i += 1) fireEvent.keyDown(seek, { key: "ArrowRight" });
    expect(seek).toHaveAttribute("aria-valuenow", "40000");
    fireEvent.keyDown(seek, { key: "Home" });
    expect(seek).toHaveAttribute("aria-valuenow", "0");
    fireEvent.keyDown(seek, { key: "End" });
    expect(seek).toHaveAttribute("aria-valuenow", "40000");
    // The position chosen before Play is applied when the file knows its length.
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "currentTime", { value: 0, configurable: true, writable: true });
    fireEvent.loadedMetadata(audio);
    expect(audio.currentTime).toBe(40);
  });

  it("a click on the waveform seeks the playing file to the point under the pointer", () => {
    const { audio, wave } = show(NOTE());
    wave.getBoundingClientRect = () => rect(100, 0, 200, 30);
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "currentTime", { value: 1, configurable: true, writable: true });
    fireEvent.pointerDown(wave, { clientX: 150, button: 0, pointerId: 1 });
    // The fill follows the pointer at once; the file is moved when the button is released.
    expect(progress(wave)).toBe("25.00%");
    fireEvent.pointerUp(wave, { clientX: 150, button: 0, pointerId: 1 });
    expect(audio.currentTime).toBe(10);
  });

  it("dragging scrubs the fill and seeks once, where the pointer is released; the frame loop does not fight the drag", () => {
    const { audio, wave } = show(NOTE());
    wave.getBoundingClientRect = () => rect(0, 0, 400, 30);
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "currentTime", { value: 1, configurable: true, writable: true });
    fireEvent.pointerDown(wave, { clientX: 40, button: 0, pointerId: 1 });
    fireEvent.pointerMove(wave, { clientX: 200, button: 0, pointerId: 1 });
    expect(progress(wave)).toBe("50.00%");
    // A frame while dragging leaves the fill under the pointer.
    frame(audio, 1.1);
    expect(progress(wave)).toBe("50.00%");
    fireEvent.pointerMove(wave, { clientX: 300, button: 0, pointerId: 1 });
    fireEvent.pointerUp(wave, { clientX: 300, button: 0, pointerId: 1 });
    expect(audio.currentTime).toBe(30);
    // A move without the button down changes nothing.
    fireEvent.pointerMove(wave, { clientX: 20, button: 0, pointerId: 1 });
    expect(audio.currentTime).toBe(30);
  });

  it("a seek before Play shows the position and waits for the file", () => {
    const { audio, wave } = show(NOTE());
    wave.getBoundingClientRect = () => rect(0, 0, 200, 30);
    fireEvent.pointerDown(wave, { clientX: 100, button: 0, pointerId: 1 });
    fireEvent.pointerUp(wave, { clientX: 100, button: 0, pointerId: 1 });
    expect(screen.getByRole("slider", { name: "Position" })).toHaveAttribute("aria-valuenow", "20000");
    expect(audio.getAttribute("src")).toBeNull();
    expect(played).toHaveLength(0);
  });

  it("a note of unknown length does not seek, and a right click never does", () => {
    const { wave } = show(demoVoice("tg", null, { mime: "audio/ogg", filename: "audio_1.ogg", byteSize: 20_700 }));
    wave.getBoundingClientRect = () => rect(0, 0, 200, 30);
    fireEvent.pointerDown(wave, { clientX: 100, button: 0, pointerId: 1 });
    fireEvent.pointerDown(wave, { clientX: 100, button: 2, pointerId: 1 });
    expect(screen.getByRole("slider", { name: "Position" })).toHaveAttribute("aria-valuenow", "0");
    expect(screen.getByRole("slider", { name: "Position" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("slider", { name: "Position" })).toHaveAttribute("tabindex", "-1");
  });
});

describe("the length and the other states", () => {
  it("takes the total from the file when the row has no length, and shows a dash until then", () => {
    const { audio } = show(demoVoice("tg", null, { mime: "audio/ogg", filename: "audio_1.ogg", byteSize: 20_700 }));
    expect(screen.getByRole("group", { name: "Voice note" })).toBeInTheDocument();
    expect(screen.getByText(/^–:––, /)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "duration", { value: 12.4, configurable: true });
    fireEvent.loadedMetadata(audio);
    expect(screen.getByRole("slider", { name: "Position" })).toHaveAttribute("aria-valuemax", "12400");
    expect(screen.getByRole("slider", { name: "Position" })).toHaveAttribute("aria-valuetext", "0:00 of 0:12");
  });

  it("an infinite duration reported by the file stays a dash", () => {
    const { audio } = show(demoVoice("tg", null, { mime: "audio/webm", filename: "v.webm" }));
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "duration", { value: Infinity, configurable: true });
    fireEvent.loadedMetadata(audio);
    expect(screen.getByRole("slider", { name: "Position" })).toHaveAttribute("aria-valuetext", "0:00 of –:––");
  });

  it("a file that is gone shows the slice 7 tile and plays nothing", () => {
    const { container } = show(demoVoice("gone", 40_000, { state: "missing" }));
    expect(container.querySelector("audio")).toBeNull();
    expect(screen.getByText("File is missing")).toBeInTheDocument();
  });

  it("a type this web view cannot play says so and offers the default app", () => {
    canPlay = "";
    const { onOpenFile, container } = show(demoVoice("ogg", 7_000, { mime: "audio/ogg", filename: "audio_2.ogg" }));
    expect(container.querySelector("audio")).toBeNull();
    expect(screen.getByText("Can't play here")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open in default app" }));
    expect(onOpenFile).toHaveBeenCalledWith("ogg");
  });

  it("a Telegram voice (Opus in Ogg) is playable when the web view plays Opus in Ogg, even if plain audio/ogg is refused", () => {
    vi.mocked(HTMLMediaElement.prototype.canPlayType).mockImplementation((type) => (type === "audio/ogg; codecs=opus" ? "probably" : ""));
    show(demoVoice("ogg", 7_000, { mime: "audio/ogg", filename: "audio_2.ogg" }));
    expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
    expect(screen.queryByText("Can't play here")).toBeNull();
  });

  it("a file over 64 MiB is not offered to the player either", () => {
    show(demoVoice("big", 3_600_000, { byteSize: 65 * 1024 * 1024 }));
    expect(screen.getByText("Can't play here")).toBeInTheDocument();
  });

  it("a load error after Play turns the player into the same message", () => {
    const { audio } = show(NOTE());
    // Before Play the element has no source: an error event means nothing.
    fireEvent.error(audio);
    expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    fireEvent.error(audio);
    expect(screen.getByText("Can't play here")).toBeInTheDocument();
  });

  it("showsWaveform says which notes get the waveform layout (the bubble puts its time on the meta line only for those)", () => {
    expect(showsWaveform(NOTE())).toBe(true);
    expect(showsWaveform(demoVoice("gone", 1000, { state: "missing" }))).toBe(false);
    expect(showsWaveform(demoVoice("big", 1000, { byteSize: 65 * 1024 * 1024 }))).toBe(false);
    canPlay = "";
    resetPlayableCache();
    expect(showsWaveform(demoVoice("ogg", 1000, { mime: "audio/ogg" }))).toBe(false);
  });
});
