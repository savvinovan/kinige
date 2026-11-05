import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen } from "../../test/render";
import { rect } from "../../test/rect";
import { installWaveCanvas } from "../../test/waveCanvas";
import { demoVoice } from "../attachments/demo";
import { frame, harness, installPlayerMocks, NOTE, progress, show } from "./playerHarness";

// The fill of the waveform while a note plays and the seeking of the slider (slice 7a). The fill is drawn on a canvas by the animation frames
// of a playing note; `data-progress` on the slider carries the share that was painted.

let canvasFills: ReturnType<typeof installWaveCanvas>;

beforeEach(() => {
  installPlayerMocks();
  canvasFills = installWaveCanvas();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(document, "hidden");
});

const { played } = harness;

describe("the fill follows an animation frame while playing", () => {
  it("runs a frame loop while playing, moves the fill inside a second and re-renders only when the whole second changes", () => {
    const { audio, wave } = show(NOTE());
    expect(harness.frames.size).toBe(0);
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(harness.frames.size).toBe(1);
    frame(audio, 2.0);
    expect(progress(wave)).toBeCloseTo(0.05, 4);
    expect(screen.getByText(/^0:02, /)).toBeInTheDocument();
    // The media element's own `timeupdate` has not fired (it does about four times a second); the frames move the fill on their own.
    const before = progress(wave);
    frame(audio, 2.04);
    expect(progress(wave)).toBeCloseTo(0.051, 4);
    expect(progress(wave)).not.toBe(before);
    frame(audio, 2.5);
    expect(progress(wave)).toBeCloseTo(0.0625, 4);
    expect(screen.getByText(/^0:02, /)).toBeInTheDocument();
    // Every frame asks for the next one.
    expect(harness.frames.size).toBe(1);
    frame(audio, 3.01);
    expect(screen.getByText(/^0:03, /)).toBeInTheDocument();
  });

  it("the fill can stop inside a bar (a share of a bar, not only whole bars)", () => {
    const { audio, wave } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    // A bar is 1/64 of the width = 1.5625 %: 3.30 % is a bar and a half.
    frame(audio, 1.32);
    const value = progress(wave);
    expect(value).toBeCloseTo(0.033, 5);
    expect((value / (1 / 64)) % 1).not.toBe(0);
    // On the canvas the played copy of the row is cut at that share of its width (191 device pixels at density 1): a bar starts every 3 pixels
    // and the cut, at 6.3 pixels, is inside the third bar.
    const played = canvasFills.fills.at(-1);
    expect(played?.style).toBe("the-accent-colour");
    expect(played?.clip).toBeCloseTo(191 * 0.033, 3);
    expect(((played?.clip ?? 0) / 3) % 1).not.toBe(0);
  });

  it("repaints the canvas on every frame, not only when the second changes", () => {
    const { audio } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    const before = canvasFills.fills.length;
    frame(audio, 2.0);
    frame(audio, 2.02);
    frame(audio, 2.04);
    // Two fills (the row and its played copy) per frame.
    expect(canvasFills.fills.length - before).toBe(6);
  });

  it("stops the loop when the note is paused", () => {
    const { audio, wave } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    frame(audio, 4);
    expect(harness.frames.size).toBe(1);
    Object.defineProperty(audio, "currentTime", { value: 4.2, configurable: true, writable: true });
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(harness.frames.size).toBe(0);
    // The paused position is painted once, from the element.
    expect(progress(wave)).toBeCloseTo(0.105, 4);
  });

  it("stops the loop when the note ends and rewinds the fill", () => {
    const { audio, wave } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    frame(audio, 39.9);
    fireEvent.ended(audio);
    expect(harness.frames.size).toBe(0);
    expect(progress(wave)).toBeCloseTo(0, 4);
    expect(screen.getByText(/^0:40, /)).toBeInTheDocument();
  });

  it("stops the loop when the player goes away", () => {
    const { unmount } = show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(harness.frames.size).toBe(1);
    unmount();
    expect(harness.frames.size).toBe(0);
  });

  it("stops the loop while the page is hidden and starts it again when it shows", () => {
    show(NOTE());
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(harness.frames.size).toBe(1);
    Object.defineProperty(document, "hidden", { value: true, configurable: true });
    document.dispatchEvent(new Event("visibilitychange"));
    expect(harness.frames.size).toBe(0);
    Object.defineProperty(document, "hidden", { value: false, configurable: true });
    document.dispatchEvent(new Event("visibilitychange"));
    expect(harness.frames.size).toBe(1);
  });

  it("a note that is only at rest never schedules a frame", () => {
    show(NOTE());
    show(demoVoice("b", 9_000));
    expect(harness.frames.size).toBe(0);
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
    expect(progress(wave)).toBeCloseTo(0.25, 4);
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
    expect(progress(wave)).toBeCloseTo(0.5, 4);
    // A frame while dragging leaves the fill under the pointer.
    frame(audio, 1.1);
    expect(progress(wave)).toBeCloseTo(0.5, 4);
    fireEvent.pointerMove(wave, { clientX: 300, button: 0, pointerId: 1 });
    fireEvent.pointerUp(wave, { clientX: 300, button: 0, pointerId: 1 });
    expect(audio.currentTime).toBe(30);
    // A move without the button down changes nothing.
    fireEvent.pointerMove(wave, { clientX: 20, button: 0, pointerId: 1 });
    expect(audio.currentTime).toBe(30);
  });

  it("a cancelled drag (pointercancel) does not seek: the note and the fill go back to where they were", () => {
    const { audio, wave } = show(NOTE());
    wave.getBoundingClientRect = () => rect(0, 0, 400, 30);
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "currentTime", { value: 4, configurable: true, writable: true });
    frame(audio, 4);
    fireEvent.pointerDown(wave, { clientX: 300, button: 0, pointerId: 1 });
    fireEvent.pointerMove(wave, { clientX: 360, button: 0, pointerId: 1 });
    expect(progress(wave)).toBeCloseTo(0.9, 4);
    fireEvent.pointerCancel(wave, { clientX: 360, button: 0, pointerId: 1 });
    expect(audio.currentTime).toBe(4);
    expect(progress(wave)).toBeCloseTo(0.1, 4);
    expect(screen.getByRole("slider", { name: "Position" })).toHaveAttribute("aria-valuenow", "4000");
    // The loop goes on from the real position, and a later move without a drag changes nothing.
    fireEvent.pointerMove(wave, { clientX: 20, button: 0, pointerId: 1 });
    expect(progress(wave)).toBeCloseTo(0.1, 4);
  });

  it("a drag that loses the pointer capture without a pointerup does not seek either; the capture lost after a pointerup is not a cancel", () => {
    const { audio, wave } = show(NOTE());
    wave.getBoundingClientRect = () => rect(0, 0, 400, 30);
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    Object.defineProperty(audio, "currentTime", { value: 4, configurable: true, writable: true });
    fireEvent.pointerDown(wave, { clientX: 300, button: 0, pointerId: 1 });
    fireEvent.lostPointerCapture(wave, { pointerId: 1 });
    expect(audio.currentTime).toBe(4);
    // A normal drag: the button is released, then the browser reports the capture lost: the seek stands.
    fireEvent.pointerDown(wave, { clientX: 100, button: 0, pointerId: 1 });
    fireEvent.pointerUp(wave, { clientX: 100, button: 0, pointerId: 1 });
    fireEvent.lostPointerCapture(wave, { pointerId: 1 });
    expect(audio.currentTime).toBe(10);
    expect(progress(wave)).toBeCloseTo(0.25, 4);
  });

  it("Up and Down step like Right and Left, PageUp and PageDown jump a tenth of the note", () => {
    show(NOTE());
    const seek = screen.getByRole("slider", { name: "Position" });
    fireEvent.keyDown(seek, { key: "ArrowUp" });
    expect(seek).toHaveAttribute("aria-valuenow", "5000");
    fireEvent.keyDown(seek, { key: "ArrowDown" });
    expect(seek).toHaveAttribute("aria-valuenow", "0");
    fireEvent.keyDown(seek, { key: "PageUp" });
    expect(seek).toHaveAttribute("aria-valuenow", "4000");
    fireEvent.keyDown(seek, { key: "PageUp" });
    fireEvent.keyDown(seek, { key: "PageUp" });
    expect(seek).toHaveAttribute("aria-valuenow", "12000");
    fireEvent.keyDown(seek, { key: "PageDown" });
    expect(seek).toHaveAttribute("aria-valuenow", "8000");
    for (let i = 0; i < 5; i += 1) fireEvent.keyDown(seek, { key: "PageDown" });
    expect(seek).toHaveAttribute("aria-valuenow", "0");
    for (let i = 0; i < 12; i += 1) fireEvent.keyDown(seek, { key: "PageUp" });
    expect(seek).toHaveAttribute("aria-valuenow", "40000");
    // Other keys are left alone (Tab must still move the focus).
    const handled = fireEvent.keyDown(seek, { key: "Tab" });
    expect(handled).toBe(true);
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
