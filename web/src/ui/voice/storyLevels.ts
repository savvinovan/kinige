// A fixed, plausible run of microphone levels (phrases and pauses) for stories: no randomness, so the snapshots do not change from run to run.
// The newest level is last.
const SAMPLE = Array.from({ length: 240 }, (_, i) => {
  const phrase = 0.5 + 0.5 * Math.sin(i * 0.11 + 1);
  const syllable = Math.abs(Math.sin(i * 0.53) * Math.cos(i * 0.29 + 2));
  return Math.min(1, 0.04 + 0.5 * phrase * syllable);
});

/** The waveform is not moving (`at: 0` is long ago, so the strip has finished sliding); the real one slides in from the right. */
export const storyLevels = (count: number) => ({ levels: SAMPLE.slice(-count), at: 0, total: SAMPLE.length });
