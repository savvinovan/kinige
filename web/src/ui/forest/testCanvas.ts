import { vi } from "vitest";
import { rect } from "../../test/rect";

/** A 2D context that accepts every call and remembers nothing: jsdom has no canvas, the engine only needs something to draw on. */
function fakeContext(): CanvasRenderingContext2D {
  const target: Record<string, unknown> = { getTransform: () => ({ a: 1 }) };
  return new Proxy(target, {
    get: (object, key) => (key in object ? object[key as string] : () => undefined),
    set: () => true,
  }) as unknown as CanvasRenderingContext2D;
}

/**
 * Gives every canvas of the test a 2D context and the board canvas a size (CSS pixels), so the engine lays out a camera and
 * draws frames. Call in `beforeEach`; the spies are restored with `vi.restoreAllMocks()`.
 */
export function stubCanvas(width = 1280, height = 360): void {
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(((kind: string) => (kind === "2d" ? fakeContext() : null)) as never);
  vi.spyOn(HTMLCanvasElement.prototype, "getBoundingClientRect").mockImplementation(() => rect(0, 0, width, height));
}
