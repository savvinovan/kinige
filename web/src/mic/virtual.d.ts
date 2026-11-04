// Resolved by `resolve.alias` in `web/vite.config.ts`: the real microphone, or the runners' fake one (KINIGE_FAKE_MIC=1).
declare module "virtual:kinige-mic" {
  export function openMicrophone(): Promise<MediaStream>;
}

// The runners' probe (KINIGE_PROBE=1) or nothing.
declare module "virtual:kinige-probe" {
  export const startProbe: ((report: (text: string) => Promise<unknown>) => void) | undefined;
}
