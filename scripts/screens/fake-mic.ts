// The fake microphone of the screens runners (slice 7a, ADR-0015). `web/vite.config.ts` points `virtual:kinige-mic` here only when
// the build is started with KINIGE_FAKE_MIC=1; otherwise this file is not in the production module graph at all. A tone from an
// `OscillatorNode` goes through a `MediaStreamAudioDestinationNode`, so the real MediaRecorder and the whole recording pipeline
// run with no device and no TCC prompt on a CI runner.
//
// The marker below is a string literal that survives minification and is written to the document at load. `build-app.sh` and
// `build-windows.sh` search `web/dist` for it and refuse to build a bundle that has it unless KINIGE_ALLOW_FAKE_MIC=1.

export const FAKE_MIC_MARKER = "kinige-fake-microphone-7a";
document.documentElement.dataset.fakeMic = FAKE_MIC_MARKER;

export async function openMicrophone(): Promise<MediaStream> {
  // Not inside the click that asked for the microphone: on a runner without an audio device `new AudioContext()` can take very long, and an
  // accessibility press (the scenarios' only input) waits for the click handler to return (the 7a run: the first press timed out at 60 s).
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  const context = new AudioContext();
  const oscillator = context.createOscillator();
  oscillator.frequency.value = 440;
  const gain = context.createGain();
  gain.gain.value = 0.2;
  const destination = context.createMediaStreamDestination();
  oscillator.connect(gain).connect(destination);
  oscillator.start();
  await context.resume().catch(() => undefined);
  for (const track of destination.stream.getAudioTracks()) {
    const stop = track.stop.bind(track);
    track.stop = () => {
      stop();
      void context.close().catch(() => undefined);
    };
  }
  return destination.stream;
}
