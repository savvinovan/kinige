import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { fireEvent, render, screen } from "../../test/render";
import { demoVoice } from "../attachments/demo";
import { MediaSourceContext } from "../attachments/mediaUrl";
import { resetPlayerSession } from "./playerSession";
import { resetPlayableCache, VoicePlayer } from "./VoicePlayer";

const SOURCE = (variant: string, id: string) => `kinige-media://localhost/${variant}/${id}`;

let canPlay = "maybe";
const played: HTMLMediaElement[] = [];
const paused: HTMLMediaElement[] = [];

beforeEach(() => {
  resetPlayerSession();
  resetPlayableCache();
  canPlay = "maybe";
  played.length = 0;
  paused.length = 0;
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
  return { ...result, audio, onOpenFile };
}

function tick(element: HTMLMediaElement, seconds: number) {
  Object.defineProperty(element, "currentTime", { value: seconds, configurable: true, writable: true });
  fireEvent.timeUpdate(element);
}

describe("VoicePlayer", () => {
  it("is labelled for the screen reader and opens a chat without asking for any audio", () => {
    const { audio } = show(demoVoice("v1", 40_000));
    expect(screen.getByRole("group", { name: "Voice note, 40 seconds" })).toBeInTheDocument();
    expect(audio.getAttribute("src")).toBeNull();
    expect(audio.preload).toBe("none");
    expect(played).toHaveLength(0);
    expect(screen.getByText("0:40")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Playback speed 1×" })).toHaveTextContent("1×");
  });

  it("is labelled in Russian with the right plural", () => {
    show(demoVoice("v1", 40_000), vi.fn(), "ru");
    expect(screen.getByRole("group", { name: "Голосовое сообщение, 40 секунд" })).toBeInTheDocument();
  });

  it("sets the source only on the first Play, then shows the position over the total", () => {
    const { audio } = show(demoVoice("v1", 40_000));
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(audio.getAttribute("src")).toBe("kinige-media://localhost/audio/v1");
    expect(played).toEqual([audio]);
    expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
    tick(audio, 3);
    expect(screen.getByText("0:03 / 0:40")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(paused).toEqual([audio]);
    expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
    // A second Play does not change the source.
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(audio.getAttribute("src")).toBe("kinige-media://localhost/audio/v1");
  });

  it("an hour or more reads as h:mm:ss", () => {
    const { audio } = show(demoVoice("long", 3_723_000));
    expect(screen.getByText("1:02:03")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    tick(audio, 3_661);
    expect(screen.getByText("1:01:01 / 1:02:03")).toBeInTheDocument();
  });

  it("cycles 1× → 1.5× → 2× → 1× and keeps the speed for the next player of the session", () => {
    const { audio, unmount } = show(demoVoice("v1", 40_000));
    const chip = () => screen.getByRole("button", { name: /^Playback speed/ });
    fireEvent.click(chip());
    expect(chip()).toHaveTextContent("1.5×");
    fireEvent.click(chip());
    expect(chip()).toHaveTextContent("2×");
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(audio.playbackRate).toBe(2);
    fireEvent.click(chip());
    expect(chip()).toHaveTextContent("1×");
    fireEvent.click(chip());
    unmount();
    show(demoVoice("v2", 10_000));
    expect(screen.getByRole("button", { name: /^Playback speed/ })).toHaveTextContent("1.5×");
  });

  it("starting another voice note pauses the first one", () => {
    const first = show(demoVoice("a", 10_000));
    const second = show(demoVoice("b", 20_000));
    const [playA, playB] = screen.getAllByRole("button", { name: "Play" });
    fireEvent.click(playA as HTMLElement);
    expect(played).toEqual([first.audio]);
    fireEvent.click(playB as HTMLElement);
    expect(paused).toContain(first.audio);
    expect(played).toEqual([first.audio, second.audio]);
    expect(screen.getAllByRole("button", { name: "Play" })).toHaveLength(1);
  });

  it("the arrow keys move the position by five seconds, and a seek before Play waits for the file", () => {
    const { audio } = show(demoVoice("v1", 40_000));
    const seek = screen.getByRole("slider", { name: "Position" });
    fireEvent.keyDown(seek, { key: "ArrowRight" });
    expect(screen.getByRole("slider", { name: "Position" })).toHaveValue("5000");
    expect(audio.getAttribute("src")).toBeNull();
    fireEvent.keyDown(seek, { key: "ArrowRight" });
    fireEvent.keyDown(seek, { key: "ArrowLeft" });
    expect(seek).toHaveValue("5000");
    fireEvent.keyDown(seek, { key: "ArrowLeft" });
    fireEvent.keyDown(seek, { key: "ArrowLeft" });
    expect(seek).toHaveValue("0");
    // At the end it stops at the total.
    for (let i = 0; i < 12; i += 1) fireEvent.keyDown(seek, { key: "ArrowRight" });
    expect(seek).toHaveValue("40000");
    // The position chosen before Play is applied when the file knows its length.
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "currentTime", { value: 0, configurable: true, writable: true });
    fireEvent.loadedMetadata(audio);
    expect(audio.currentTime).toBe(40);
  });

  it("dragging the bar seeks the playing file", () => {
    const { audio } = show(demoVoice("v1", 40_000));
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "currentTime", { value: 1, configurable: true, writable: true });
    fireEvent.change(screen.getByRole("slider", { name: "Position" }), { target: { value: "12000" } });
    expect(audio.currentTime).toBe(12);
  });

  it("takes the total from the file when the row has no length, and shows a dash until then", () => {
    const { audio } = show(demoVoice("tg", null, { mime: "audio/ogg", filename: "audio_1.ogg" }));
    expect(screen.getByRole("group", { name: "Voice note" })).toBeInTheDocument();
    expect(screen.getByText("–:––")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "duration", { value: 12.4, configurable: true });
    fireEvent.loadedMetadata(audio);
    expect(screen.getByText("0:00 / 0:12")).toBeInTheDocument();
    // An infinite duration (a stream without a header) stays a dash.
  });

  it("an infinite duration reported by the file stays a dash", () => {
    const { audio } = show(demoVoice("tg", null, { mime: "audio/webm", filename: "v.webm" }));
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "duration", { value: Infinity, configurable: true });
    fireEvent.loadedMetadata(audio);
    expect(screen.getByText("0:00 / –:––")).toBeInTheDocument();
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

  it("a file over 64 MiB is not offered to the player either", () => {
    show(demoVoice("big", 3_600_000, { byteSize: 65 * 1024 * 1024 }));
    expect(screen.getByText("Can't play here")).toBeInTheDocument();
  });

  it("a load error after Play turns the player into the same message", () => {
    const { audio } = show(demoVoice("v1", 40_000));
    // Before Play the element has no source: an error event means nothing.
    fireEvent.error(audio);
    expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    fireEvent.error(audio);
    expect(screen.getByText("Can't play here")).toBeInTheDocument();
  });

  it("playing to the end rewinds and releases the playback", () => {
    const { audio } = show(demoVoice("v1", 40_000));
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    tick(audio, 39);
    fireEvent.ended(audio);
    expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
    expect(screen.getByText("0:00 / 0:40")).toBeInTheDocument();
  });
});
