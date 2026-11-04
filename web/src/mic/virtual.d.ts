// Resolved by `resolve.alias` in `web/vite.config.ts`: the real microphone, or the runners' fake one (KINIGE_FAKE_MIC=1).
declare module "virtual:kinige-mic" {
  export function openMicrophone(): Promise<MediaStream>;
  /** When a recording is sent by itself, if sooner than the product limit; `null` in a normal build. */
  export const recordingLimitMs: number | null;
}

// The runners' probe (KINIGE_PROBE=1) or nothing.
declare module "virtual:kinige-probe" {
  export const startProbe: ((report: (text: string) => Promise<unknown>) => void) | undefined;
}
