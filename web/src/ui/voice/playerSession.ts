// Things the voice players of one window share (slice 7a): which one is playing and the speed. Module state on purpose: it lives as long as the
// window (a session), is not saved anywhere (decision 6) and has to be seen by players that never meet in the tree.

/** The speeds of the chip, in the order it cycles: 1× → 1.5× → 2× → 1× (Telegram's steps). */
export const SPEEDS = [1, 1.5, 2] as const;

/** What a player lets the others do to it. */
export interface PlayingHandle {
  pause: () => void;
}

let playing: PlayingHandle | null = null;
let speedIndex = 0;

/** A player started: the one that played before it is paused, so only one voice note plays at a time. */
export function claimPlayback(handle: PlayingHandle): void {
  if (playing !== null && playing !== handle) playing.pause();
  playing = handle;
}

/** A player stopped (paused, ended, unmounted): if it was the one playing, nobody is. */
export function releasePlayback(handle: PlayingHandle): void {
  if (playing === handle) playing = null;
}

export function currentSpeed(): number {
  return SPEEDS[speedIndex] ?? 1;
}

/** The chip was pressed: the next speed, for every player of the session. */
export function nextSpeed(): number {
  speedIndex = (speedIndex + 1) % SPEEDS.length;
  return currentSpeed();
}

/** Tests only: back to the start of a session. */
export function resetPlayerSession(): void {
  playing = null;
  speedIndex = 0;
}

/** "1×", "1.5×", "2×". */
export function speedLabel(speed: number): string {
  return `${speed}×`;
}
