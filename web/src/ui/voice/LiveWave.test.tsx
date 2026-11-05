import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { rect } from "../../test/rect";
import { act, render, screen } from "../../test/render";
import { LEVEL_PERIOD_MS } from "../../voice/recorderSession";
import { LiveWave } from "./LiveWave";

// A canvas that remembers what is drawn, animation frames the test runs by hand and token lengths from a fake computed style.

let frames = new Map<number, FrameRequestCallback>();
let nextFrame = 1;
let moves: number[] = [];
let fills = 0;
let observers: { disconnected: boolean; callback: () => void }[] = [];

// The guard on raw values reads tests too: units are written from parts, the way the stylesheet's tokens reach the code.
const rem = (amount: number) => `${amount}rem`;
const px = (amount: number) => `${amount}px`;

const TOKENS: Record<string, string> = {
  "--layout-voice-rec-wave-bar-width": rem(0.2),
  "--layout-voice-rec-wave-gap": rem(0.15),
  "--layout-voice-rec-wave-min-height": rem(0.2),
  "--layout-voice-rec-wave-height": rem(1.6),
};

/** A computed style with the token lengths, a colour and a root font size. */
function style(rootPx: number) {
  return () =>
    ({ getPropertyValue: (name: string) => TOKENS[name] ?? "", color: "the-accent-colour", fontSize: px(rootPx) }) as unknown as CSSStyleDeclaration;
}

beforeEach(() => {
  frames = new Map();
  nextFrame = 1;
  moves = [];
  fills = 0;
  observers = [];
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    frames.set(nextFrame, callback);
    return nextFrame++;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
  vi.stubGlobal(
    "ResizeObserver",
    class {
      readonly state: { disconnected: boolean; callback: () => void };
      constructor(callback: () => void) {
        this.state = { disconnected: false, callback };
        observers.push(this.state);
      }
      observe() {}
      disconnect() {
        this.state.disconnected = true;
      }
    },
  );
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(
    () =>
      ({
        clearRect: () => undefined,
        beginPath: () => undefined,
        moveTo: (x: number) => moves.push(x),
        arcTo: () => undefined,
        closePath: () => undefined,
        fill: () => (fills += 1),
        fillStyle: "",
      }) as unknown as CanvasRenderingContext2D,
  );
  vi.spyOn(HTMLCanvasElement.prototype, "getBoundingClientRect").mockReturnValue(rect(0, 0, 200, 26));
  vi.spyOn(window, "getComputedStyle").mockImplementation(style(10));
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const runFrame = () => {
  const [id, callback] = [...frames.entries()][0] ?? [];
  if (id === undefined || !callback) throw new Error("no animation frame is pending");
  frames.delete(id);
  act(() => callback(performance.now()));
};

const LEVELS = { levels: [0.2, 0.4, 0.6], at: 0, total: 3 };

describe("LiveWave", () => {
  it("is one canvas with a name for the screen reader", () => {
    render(<LiveWave read={() => LEVELS} label="Microphone level" />, { language: "en" });
    expect(screen.getByRole("img", { name: "Microphone level" }).tagName).toBe("CANVAS");
  });

  it("asks for as many levels as fit (token lengths at the root size) and draws them on every animation frame", () => {
    const read = vi.fn(() => LEVELS);
    render(<LiveWave read={read} label="Microphone level" />, { language: "en" });
    expect(frames.size).toBe(1);
    runFrame();
    // 0.2 rem at 10 px is a bar of 2 device pixels, 0.15 rem (1.5) a gap of 2 (whole pixels): a pitch of 4; 200 / 4 = 50 bars, plus two.
    expect(read).toHaveBeenLastCalledWith(52);
    expect(fills).toBe(1);
    expect(moves.length).toBe(3);
    // The next frame is already scheduled: the waveform runs by itself, with no render.
    expect(frames.size).toBe(1);
    runFrame();
    expect(fills).toBe(2);
  });

  it("slides: the same levels are drawn further left a moment after the newest was taken", () => {
    const now = vi.spyOn(performance, "now");
    now.mockReturnValue(1000);
    render(<LiveWave read={() => ({ levels: [0.5, 0.5], at: 1000, total: 2 })} label="Microphone level" />, { language: "en" });
    runFrame();
    const fresh = moves[0] ?? 0;
    moves = [];
    now.mockReturnValue(1000 + LEVEL_PERIOD_MS / 2);
    runFrame();
    // At phase 0 the newest bar is outside and the first one drawn is the one before it (x 199); half a period later the newest bar has come
    // in by half a pitch (2 px): it is drawn first, at x 201 (its left side at the edge).
    expect(fresh).toBe(199);
    expect(moves[0]).toBe(201);
  });

  it("draws again at once when the box is resized (resizing a canvas clears it), not on the next frame or step", () => {
    render(<LiveWave read={() => LEVELS} label="Microphone level" />, { language: "en" });
    runFrame();
    const before = fills;
    act(() => observers[0]?.callback());
    expect(fills).toBe(before + 1);
  });

  it("stops the frames and the observer when it goes away", () => {
    const { unmount } = render(<LiveWave read={() => LEVELS} label="Microphone level" />, { language: "en" });
    runFrame();
    expect(frames.size).toBe(1);
    unmount();
    expect(frames.size).toBe(0);
    expect(observers.every((observer) => observer.disconnected)).toBe(true);
  });

  it("with Reduce Motion nothing scrolls: no animation frames, a step ten times a second, the bars in fixed slots", () => {
    vi.useFakeTimers();
    try {
      let total = 1;
      render(
        <div data-reduce-motion>
          <LiveWave read={() => ({ levels: Array.from({ length: total }, () => 0.3), at: 0, total })} label="Microphone level" />
        </div>,
        { language: "en" },
      );
      expect(frames.size).toBe(0);
      const first = fills;
      expect(first).toBe(1);
      // One bar in the first slot (the bars are whole pixels: 2 wide with gaps of 2 at 10 px root, 50 slots, the first one a radius in).
      expect(moves).toEqual([3]);
      moves = [];
      total = 2;
      act(() => {
        vi.advanceTimersByTime(LEVEL_PERIOD_MS * 2);
      });
      expect(fills).toBe(first + 1);
      // A second level takes the second slot; the first bar did not move.
      expect(moves).toEqual([3, 7]);
      act(() => {
        vi.advanceTimersByTime(LEVEL_PERIOD_MS * 2 * 2);
      });
      expect(fills).toBe(first + 3);
    } finally {
      vi.useRealTimers();
    }
  });

  it("draws nothing when the tokens cannot be read, and does not crash without a 2D context", () => {
    TOKENS["--layout-voice-rec-wave-gap"] = `calc(${px(1)} + ${px(1)})`;
    const unreadable = render(<LiveWave read={() => LEVELS} label="Microphone level" />, { language: "en" });
    runFrame();
    expect(fills).toBe(0);
    unreadable.unmount();
    TOKENS["--layout-voice-rec-wave-gap"] = rem(0.15);
    vi.mocked(HTMLCanvasElement.prototype.getContext).mockImplementation(() => null);
    render(<LiveWave read={() => LEVELS} label="Microphone level" />, { language: "en" });
    expect(frames.size).toBe(0);
  });

  it("while the tokens cannot be read yet (the stylesheet is not applied at the first frame) it looks again on every frame and draws as soon as it can", () => {
    TOKENS["--layout-voice-rec-wave-gap"] = "";
    render(<LiveWave read={() => LEVELS} label="Microphone level" />, { language: "en" });
    runFrame();
    expect(fills).toBe(0);
    TOKENS["--layout-voice-rec-wave-gap"] = rem(0.15);
    runFrame();
    expect(fills).toBe(1);
  });

  it("looks at the tokens again about once a second (the text size can change)", () => {
    const now = vi.spyOn(performance, "now");
    now.mockReturnValue(0);
    const read = vi.fn(() => LEVELS);
    render(<LiveWave read={read} label="Microphone level" />, { language: "en" });
    runFrame();
    expect(read).toHaveBeenLastCalledWith(52);
    // The root grows to 20 px: bars are twice as wide, fewer fit.
    vi.mocked(window.getComputedStyle).mockImplementation(style(20));
    now.mockReturnValue(1200);
    runFrame();
    expect(read).toHaveBeenLastCalledWith(31);
  });
});
