/** Shown for a duration that is not known: the total of a player whose row has no `durationMs` and whose file has not loaded. */
export const UNKNOWN_TIME = "–:––";

/**
 * `m:ss`, or `h:mm:ss` from one hour: the same form as `kinige_domain::attachment::format_duration` (the Markdown copy). A negative or
 * non-finite value reads as `0:00`.
 */
export function formatDuration(ms: number): string {
  const total = Number.isFinite(ms) ? Math.max(0, Math.floor(ms / 1000)) : 0;
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  const two = (value: number) => String(value).padStart(2, "0");
  return hours > 0 ? `${hours}:${two(minutes)}:${two(seconds)}` : `${minutes}:${two(seconds)}`;
}

/** Whole seconds, for the screen reader label "Voice note, 40 seconds". */
export function wholeSeconds(ms: number): number {
  return Number.isFinite(ms) ? Math.max(0, Math.round(ms / 1000)) : 0;
}
