// The growth curve comes from the design tokens: `--motion-easing-enter` is a CSS `cubic-bezier(x1, y1, x2, y2)`, and the
// canvas has to evaluate it itself.

export type Easing = (t: number) => number;

const LINEAR: Easing = (t) => Math.min(1, Math.max(0, t));

/** Four numbers from a value like `cubic-bezier(0, 0, 0.2, 1)`; `null` for anything else. */
export function parseBezier(text: string): [number, number, number, number] | null {
  const match = /^\s*cubic-bezier\(([^)]*)\)\s*$/.exec(text);
  if (!match) return null;
  const numbers = (match[1] ?? "").split(",").map((part) => Number(part.trim()));
  const [x1, y1, x2, y2] = numbers;
  if (numbers.length !== 4 || numbers.some((n) => !Number.isFinite(n))) return null;
  if (x1 === undefined || y1 === undefined || x2 === undefined || y2 === undefined) return null;
  if (x1 < 0 || x1 > 1 || x2 < 0 || x2 > 1) return null; // x must stay a function of time
  return [x1, y1, x2, y2];
}

/** The CSS timing function: progress for time `t` in 0..1. An unreadable curve falls back to linear. */
export function cubicBezier(text: string): Easing {
  const curve = parseBezier(text);
  if (!curve) return LINEAR;
  const [x1, y1, x2, y2] = curve;
  const at = (a: number, b: number, u: number) => 3 * a * (1 - u) ** 2 * u + 3 * b * (1 - u) * u ** 2 + u ** 3;
  return (t) => {
    if (t <= 0) return 0;
    if (t >= 1) return 1;
    // Bisection on the curve parameter: x(u) is monotonic for x1, x2 in 0..1.
    let lo = 0;
    let hi = 1;
    for (let i = 0; i < 24; i++) {
      const mid = (lo + hi) / 2;
      if (at(x1, x2, mid) < t) lo = mid;
      else hi = mid;
    }
    return at(y1, y2, (lo + hi) / 2);
  };
}

/** A CSS time (`0.7s`, `700ms`, `0s`) in milliseconds; `0` for anything unreadable, so the tree just jumps. */
export function parseDuration(text: string): number {
  const match = /^\s*(-?\d*\.?\d+)\s*(ms|s)\s*$/.exec(text);
  if (!match) return 0;
  const value = Number(match[1]) * (match[2] === "s" ? 1000 : 1);
  return Number.isFinite(value) && value > 0 ? value : 0;
}
