import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "../../test/render";
import { Album } from "./Album";
import { DemoMedia, demoPhoto } from "./demo";
import { RevealProvider } from "./reveal";

// GIF playback (slice 5c): the thumbnail until the cell is at least half on screen, `full` while it plays, the size rules, Reduce Motion followed live.

const MIB = 1024 * 1024;

class FakeIntersectionObserver {
  static all: FakeIntersectionObserver[] = [];
  readonly targets = new Set<Element>();
  constructor(private readonly callback: IntersectionObserverCallback) {
    FakeIntersectionObserver.all.push(this);
  }
  observe(target: Element) {
    this.targets.add(target);
  }
  unobserve(target: Element) {
    this.targets.delete(target);
  }
  disconnect() {
    this.targets.clear();
    FakeIntersectionObserver.all = FakeIntersectionObserver.all.filter((observer) => observer !== this);
  }
  takeRecords() {
    return [];
  }
  /** The browser's report for `target`: how much of it is on screen. */
  report(target: Element, ratio: number) {
    this.callback([{ target, intersectionRatio: ratio, isIntersecting: ratio > 0 } as IntersectionObserverEntry], this as unknown as IntersectionObserver);
  }
}

/** Puts `ratio` of the GIF's cell on screen (or takes it away). */
const scroll = (ratio: number, index = 0) => {
  const [observer] = FakeIntersectionObserver.all;
  const cells = [...(observer?.targets ?? [])];
  act(() => observer?.report(cells[index] as Element, ratio));
};

let reduced = false;
const motionListeners = new Set<() => void>();
const setReduced = (value: boolean) => {
  reduced = value;
  act(() => motionListeners.forEach((listener) => listener()));
};

beforeEach(() => {
  FakeIntersectionObserver.all = [];
  reduced = false;
  motionListeners.clear();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  vi.stubGlobal("matchMedia", () => ({
    get matches() {
      return reduced;
    },
    addEventListener: (_type: string, listener: () => void) => motionListeners.add(listener),
    removeEventListener: (_type: string, listener: () => void) => motionListeners.delete(listener),
  }));
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
});

const gif = (id: string, byteSize: number, width = 480, height = 270, extra: Parameters<typeof demoPhoto>[4] = {}) =>
  demoPhoto(id, `${id}.gif`, width, height, { mime: "image/gif", byteSize, ...extra });

const show = (...images: ReturnType<typeof gif>[]) => {
  const onOpen = vi.fn();
  const view = render(
    <DemoMedia>
      <RevealProvider viewKey="c">
        <Album images={images} onOpen={onOpen} />
      </RevealProvider>
    </DemoMedia>,
    { language: "en" },
  );
  return { onOpen, ...view };
};

const cells = () => screen.getAllByRole("button");
const image = (index = 0) => cells()[index]?.querySelector("img") as HTMLImageElement;
const src = (index = 0) => image(index).getAttribute("src");
const badge = (index = 0) => cells()[index]?.querySelector(".gif-badge") ?? null;
const state = (index = 0) => cells()[index]?.getAttribute("data-gif");

describe("a GIF that autoplays (at most 8 MiB and 2,000,000 pixels)", () => {
  it("starts on the thumbnail, plays `full` while at least half of the cell is on screen, and goes back to the thumbnail when it leaves", () => {
    // 1600 by 900 (1.44 million pixels): the demo thumbnail (640 wide) is not the same picture as the original, as in the app.
    show(gif("g1", 1 * MIB, 1600, 900));
    const still = src();
    expect(state()).toBe("still");
    scroll(0.49);
    expect(state()).toBe("still");
    scroll(0.5);
    expect(state()).toBe("playing");
    expect(src()).not.toBe(still);
    scroll(0);
    expect(state()).toBe("still");
    expect(src()).toBe(still);
  });

  it("has no badge, playing or waiting for the screen; a click on a playing GIF opens the viewer", () => {
    const { onOpen } = show(gif("g1", 1 * MIB));
    expect(badge()).toBeNull();
    scroll(1);
    expect(badge()).toBeNull();
    fireEvent.click(cells()[0] as HTMLElement);
    expect(onOpen).toHaveBeenCalledExactlyOnceWith(0);
    expect(state()).toBe("playing");
  });

  it("a hidden window plays nothing; the GIF starts again when the window is shown", () => {
    show(gif("g1", 1 * MIB));
    scroll(1);
    expect(state()).toBe("playing");
    act(() => {
      Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(state()).toBe("still");
    act(() => {
      Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(state()).toBe("playing");
  });

  it("the box is the same still and playing (nothing moves): one picture element, one box, only the source changes", () => {
    show(gif("g1", 1 * MIB, 480, 270));
    const picture = image();
    const box = cells()[0]?.closest(".album--single") as HTMLElement;
    const before = box.getAttribute("style");
    scroll(1);
    expect(image()).toBe(picture);
    expect(box.getAttribute("style")).toBe(before);
  });
});

describe("one shared observer", () => {
  it("any number of GIF cells are watched by one observer, which goes when the last cell does", () => {
    const view = show(gif("a", MIB), gif("b", MIB), gif("c", MIB));
    expect(FakeIntersectionObserver.all).toHaveLength(1);
    expect(FakeIntersectionObserver.all[0]?.targets.size).toBe(3);
    // Each cell is told about itself only.
    scroll(1, 1);
    expect([0, 1, 2].map((index) => state(index))).toEqual(["still", "playing", "still"]);
    view.unmount();
    expect(FakeIntersectionObserver.all).toHaveLength(0);
  });

  it("an ordinary photo is not watched at all", () => {
    show(demoPhoto("p1", "p.jpg", 100, 100));
    expect(FakeIntersectionObserver.all).toHaveLength(0);
  });
});

describe("a GIF over the size rules (up to 64 MiB): the badge, a click plays it until it leaves the screen", () => {
  it("8 MiB and one byte, or 2,000,001 pixels, do not autoplay", () => {
    show(gif("big", 8 * MIB + 1), gif("wide", MIB, 2000, 1001));
    scroll(1, 0);
    scroll(1, 1);
    expect(state(0)).toBe("still");
    expect(state(1)).toBe("still");
    expect(badge(0)).not.toBeNull();
    expect(badge(1)).not.toBeNull();
  });

  it("the edges themselves do autoplay: exactly 8 MiB, exactly 2,000,000 pixels", () => {
    show(gif("edge1", 8 * MIB), gif("edge2", MIB, 2000, 1000));
    scroll(1, 0);
    scroll(1, 1);
    expect(state(0)).toBe("playing");
    expect(state(1)).toBe("playing");
  });

  it("a click plays that GIF and hides the badge; leaving the screen stops it, and it needs a click again", () => {
    const { onOpen } = show(gif("big", 20 * MIB));
    scroll(1);
    expect(state()).toBe("still");
    fireEvent.click(cells()[0] as HTMLElement);
    expect(state()).toBe("playing");
    expect(badge()).toBeNull();
    expect(onOpen).not.toHaveBeenCalled();
    // A click on the playing GIF opens the viewer.
    fireEvent.click(cells()[0] as HTMLElement);
    expect(onOpen).toHaveBeenCalledOnce();
    scroll(0);
    expect(state()).toBe("still");
    scroll(1);
    expect(state()).toBe("still");
    expect(badge()).not.toBeNull();
  });

  it("the label says a click plays it", () => {
    show(gif("big", 20 * MIB));
    expect(cells()[0]).toHaveAccessibleName(/Press to play the GIF$/);
  });
});

describe("a GIF over 64 MiB", () => {
  it("never plays, keeps the badge, and a click opens the viewer", () => {
    const { onOpen } = show(gif("huge", 64 * MIB + 1));
    scroll(1);
    expect(state()).toBe("still");
    expect(badge()).not.toBeNull();
    fireEvent.click(cells()[0] as HTMLElement);
    expect(state()).toBe("still");
    expect(onOpen).toHaveBeenCalledOnce();
  });
});

describe("Reduce Motion, followed live", () => {
  it("every GIF stands still with the badge; a click plays that one; turning it off lets the others play", () => {
    show(gif("a", MIB), gif("b", MIB));
    setReduced(true);
    scroll(1, 0);
    scroll(1, 1);
    expect([state(0), state(1)]).toEqual(["still", "still"]);
    expect(badge(0)).not.toBeNull();
    fireEvent.click(cells()[0] as HTMLElement);
    expect([state(0), state(1)]).toEqual(["playing", "still"]);
    setReduced(false);
    expect([state(0), state(1)]).toEqual(["playing", "playing"]);
    expect(badge(1)).toBeNull();
  });

  it("turning it on stops a GIF that is playing, at once", () => {
    show(gif("a", MIB));
    scroll(1);
    expect(state()).toBe("playing");
    setReduced(true);
    expect(state()).toBe("still");
    expect(badge()).not.toBeNull();
  });

  it("the data-reduce-motion attribute of the window counts, like the OS setting", async () => {
    show(gif("a", MIB));
    scroll(1);
    expect(state()).toBe("playing");
    document.documentElement.setAttribute("data-reduce-motion", "");
    await act(async () => undefined);
    expect(state()).toBe("still");
    document.documentElement.removeAttribute("data-reduce-motion");
    await act(async () => undefined);
    expect(state()).toBe("playing");
  });
});

describe("a covered GIF", () => {
  it("loads nothing and is not watched until it is revealed", () => {
    show(gif("secret", MIB, 480, 270, { spoiler: true }));
    expect(document.querySelector("img")).toBeNull();
    expect(FakeIntersectionObserver.all).toHaveLength(0);
    fireEvent.click(screen.getByRole("button", { name: "Hidden photo, press to show" }));
    expect(document.querySelector("img")).not.toBeNull();
    expect(FakeIntersectionObserver.all).toHaveLength(1);
    scroll(1);
    expect(state()).toBe("playing");
  });
});

describe("failures", () => {
  it("a thumbnail that cannot be decoded is the 'No preview' tile, as for any photo", () => {
    show(gif("g1", MIB));
    fireEvent.error(image());
    expect(screen.getByText("No preview")).toBeInTheDocument();
  });

  it("a `full` that cannot be decoded goes back to the thumbnail and the badge; a click then opens the viewer", () => {
    const { onOpen } = show(gif("g1", MIB));
    const still = src();
    scroll(1);
    fireEvent.error(image());
    expect(state()).toBe("still");
    expect(src()).toBe(still);
    expect(badge()).not.toBeNull();
    fireEvent.click(cells()[0] as HTMLElement);
    expect(onOpen).toHaveBeenCalledOnce();
  });

  it("a missing file is the 'File is missing' tile and watches nothing", () => {
    show(gif("g1", MIB, 480, 270, { missing: true }));
    expect(FakeIntersectionObserver.all).toHaveLength(0);
    expect(screen.getByRole("img", { name: /File is missing/ })).toBeInTheDocument();
  });
});

describe("without IntersectionObserver", () => {
  it("nothing plays, and the GIF still opens in the viewer", () => {
    vi.stubGlobal("IntersectionObserver", undefined);
    const { onOpen } = show(gif("g1", MIB));
    expect(state()).toBe("still");
    fireEvent.click(cells()[0] as HTMLElement);
    expect(onOpen).toHaveBeenCalledOnce();
  });
});
