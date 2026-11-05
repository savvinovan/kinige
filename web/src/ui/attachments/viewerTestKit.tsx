import { useState } from "react";
import { afterEach, beforeEach, vi } from "vitest";
import { fireEvent, screen } from "../../test/render";
import { rect } from "../../test/rect";
import { demoPhoto } from "./demo";
import { Lightbox } from "./Lightbox";
import { MediaSourceContext } from "./mediaUrl";

// Shared by the viewer tests (KINIG-87: `LightboxClose`, `LightboxZoom`, `LightboxRotate`). jsdom has no layout: the stage (the figure) is
// 1000 x 600 at (100, 60), so its centre is (600, 360); a 1600 x 900 photo fits it at 0.625 (a 1000 x 562.5 box), turned a quarter at
// 0.375 (337.5 x 600).

export const STAGE = rect(100, 60, 1000, 600);
export const BOX = [1000, 562.5] as const;
export const CENTRE = { clientX: 600, clientY: 360 };
export const WIDE = demoPhoto("wide", "IMG_wide.jpg", 1600, 900);
export const TALL = demoPhoto("tall", "IMG_tall.jpg", 900, 1600);
export const BAD = demoPhoto("bad", "IMG_bad.jpg", 1280, 960, { broken: true });

type Point = { clientX: number; clientY: number };
const source = (variant: string, id: string) => `test://${variant}/${id}`;

export function Harness({ images = [WIDE, TALL, WIDE], start = 0, onClose = vi.fn() }) {
  const [index, setIndex] = useState(start);
  return (
    <MediaSourceContext.Provider value={source}>
      <Lightbox images={images} index={index} onIndexChange={setIndex} onClose={onClose} onOpenExternal={vi.fn()} />
    </MediaSourceContext.Provider>
  );
}

export const key = (name: string, init: KeyboardEventInit = {}) => fireEvent.keyDown(document, { key: name, ...init });
export const image = () => screen.getByRole("img");
export const dialog = () => screen.getByRole("dialog");
export const stage = () => document.querySelector<HTMLElement>(".lightbox__stage") as HTMLElement;
export const percent = () => screen.getByRole("button", { name: /^Fit to window, zoom/ }).textContent;

/** What the photo's transform says: the pan, the scale and the turn in degrees. */
export function view() {
  const transform = image().style.transform;
  const number = (pattern: RegExp) => Number(pattern.exec(transform)?.[1]);
  const pan = /translate\((-?[\d.]+)px, (-?[\d.]+)px\)/.exec(transform);
  return { x: Number(pan?.[1]), y: Number(pan?.[2]), scale: number(/scale\((-?[\d.]+)\)/), turn: number(/rotate\((-?[\d.]+)deg\)/) };
}

/** A press and release at one point: a click as the viewer reads it (the pointer events, the browser's `click` comes after). */
export function click(target: Element, at: Point = CENTRE) {
  const init = { ...at, button: 0, pointerId: 1 };
  fireEvent.pointerDown(target, init);
  fireEvent.pointerUp(target, init);
}

export function drag(target: Element, from: Point, to: Point) {
  fireEvent.pointerDown(target, { ...from, button: 0, pointerId: 1 });
  fireEvent.pointerMove(target, { ...to, pointerId: 1 });
  fireEvent.pointerUp(target, { ...to, pointerId: 1 });
}

export const wheel = (init: WheelEventInit) => fireEvent.wheel(dialog(), { ...CENTRE, ...init });

/** Fake timers (the close waits for a double click) and the measured stage, for every test of the file that calls it. */
export function withViewerStage() {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      return this.classList.contains("lightbox__figure") ? STAGE : rect(0, 0, 1200, 800);
    });
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });
}
