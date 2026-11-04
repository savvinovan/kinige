import type { VoiceController, VoiceState } from "./useVoiceRecorder";

/** Storybook and tests: a microphone frozen in one state, with the given functions (or none) behind the buttons. */
export function staticVoice(state: VoiceState, handlers: Partial<Omit<VoiceController, "store">> = {}): VoiceController {
  const noop = () => undefined;
  return {
    store: { get: () => state, subscribe: () => () => undefined },
    levels: () => [],
    start: noop,
    send: noop,
    requestCancel: noop,
    keepRecording: noop,
    discard: noop,
    retry: noop,
    openSettings: noop,
    dismissNotice: noop,
    ...handlers,
  };
}
