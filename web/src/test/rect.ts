/** A rectangle for replacing `getBoundingClientRect` in jsdom: there are no real bounds there. */
export function rect(left: number, top: number, width: number, height: number): DOMRect {
  const right = left + width;
  const bottom = top + height;
  return { left, top, right, bottom, width, height, x: left, y: top, toJSON: () => ({}) };
}
