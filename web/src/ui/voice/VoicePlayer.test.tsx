import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen } from "../../test/render";
import { installWaveCanvas } from "../../test/waveCanvas";
import { demoPeaks, demoVoice } from "../attachments/demo";
import { harness, installPlayerMocks, NOTE, progress, show, tick } from "./playerHarness";
import { resetPlayableCache, showsWaveform } from "./VoicePlayer";

beforeEach(installPlayerMocks);

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const { played, paused } = harness;

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
    expect(harness.frames.size).toBe(0);
  });

  it("is labelled in Russian with the right plural and the size in Russian units", () => {
    show(NOTE(), vi.fn(), "ru");
    expect(screen.getByRole("group", { name: "Голосовое сообщение, 40 секунд" })).toBeInTheDocument();
    expect(screen.getByText(/^0:40, 20,7 /)).toBeInTheDocument();
  });

  it("draws the 64 bars on one canvas that is exactly as wide as the bars and gaps in whole device pixels", () => {
    const { fills } = installWaveCanvas();
    const { wave, canvas } = show(NOTE());
    expect(canvas).not.toBeNull();
    expect(canvas).toHaveAttribute("aria-hidden", "true");
    // 0.16 rem and 0.11 rem at 13 px are 2.08 and 1.43 CSS pixels: at density 1 a bar of 2 and a gap of 1, 64 bars and 63 gaps in all.
    expect(canvas?.width).toBe(64 * 2 + 63);
    expect(canvas?.height).toBe(21);
    // At rest there is no played layer: every fill is the muted row of 64 bars.
    expect(fills.length).toBeGreaterThanOrEqual(1);
    for (const fill of fills) {
      expect(fill.bars).toHaveLength(64);
      expect(fill.style).toBe("the-muted-colour");
    }
    expect(progress(wave)).toBe(0);
    expect(wave.querySelectorAll("canvas")).toHaveLength(1);
  });

  it("the rhythm is even at every density: a bar and a gap are whole numbers of device pixels", () => {
    for (const ratio of [1, 1.5, 2, 3]) {
      const { fills } = installWaveCanvas({ ratio });
      const { canvas, unmount } = show(NOTE());
      const bars = fills.at(-1)?.bars ?? [];
      const steps = new Set(bars.slice(1).map((bar, i) => bar.x - (bars[i]?.x ?? 0)));
      expect(steps.size, `${ratio}`).toBe(1);
      expect(Number.isInteger([...steps][0]), `${ratio}`).toBe(true);
      expect(canvas?.style.width, `${ratio}`).toBe(`${(canvas?.width ?? 0) / ratio}px`);
      unmount();
      vi.restoreAllMocks();
      vi.unstubAllGlobals();
      installPlayerMocks();
    }
  });

  it("draws a flat row of dots while the note has no peaks, and for the marker 'could not decode', in the same layout", () => {
    const flat = (peaks: number[] | null) => {
      const { fills } = installWaveCanvas();
      const { canvas, unmount } = show(demoVoice("v1", 40_000, { byteSize: 21_196, waveform: peaks }));
      const bars = fills.at(-1)?.bars ?? [];
      const width = canvas?.width;
      unmount();
      vi.restoreAllMocks();
      vi.unstubAllGlobals();
      installPlayerMocks();
      return { heights: new Set(bars.map((bar) => bar.h)), count: bars.length, width };
    };
    const unknown = flat(null);
    const marker = flat([]);
    const real = flat(demoPeaks(1));
    expect(unknown.count).toBe(64);
    expect(unknown.heights.size).toBe(1);
    expect(marker.heights.size).toBe(1);
    expect(real.heights.size).toBeGreaterThan(5);
    // The bubble keeps its size when the peaks arrive.
    expect(unknown.width).toBe(real.width);
    expect(marker.width).toBe(real.width);
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
    harness.canPlay = "";
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
    harness.canPlay = "";
    resetPlayableCache();
    expect(showsWaveform(demoVoice("ogg", 1000, { mime: "audio/ogg" }))).toBe(false);
  });
});
