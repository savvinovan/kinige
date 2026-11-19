import { vi } from "vitest";

// A fake `MediaDevices` and `MediaRecorder` for the tests of the two recording windows: the real pipeline of the recorder runs with no device
// and no TCC. `install` before each test, `uninstall` after it.

export class FakeTrack extends EventTarget {
  stopped = false;
  stop() {
    this.stopped = true;
  }
}

export class FakeStream {
  readonly track = new FakeTrack();
  getAudioTracks() {
    return [this.track];
  }
  getTracks() {
    return [this.track];
  }
}

class FakeRecorder extends EventTarget {
  state: "inactive" | "recording" = "inactive";
  static isTypeSupported(type: string) {
    return type === "audio/mp4;codecs=mp4a.40.2";
  }
  start() {
    this.state = "recording";
  }
  stop() {
    this.state = "inactive";
    const event = new Event("dataavailable") as Event & { data: Blob };
    event.data = new Blob([new Uint8Array(16).fill(7)]);
    this.dispatchEvent(event);
    this.dispatchEvent(new Event("stop"));
  }
}

export interface FakeMedia {
  /** Every stream handed out, in order. */
  streams: FakeStream[];
  getUserMedia: ReturnType<typeof vi.fn>;
  /** The next request waits for `answer`: the system's permission prompt on the first use. */
  hold: () => { answer: () => void };
  /** The next requests are refused with this DOMException name. */
  refuse: (name: string) => void;
}

export function installFakeMedia(): FakeMedia {
  const media: FakeMedia = {
    streams: [],
    getUserMedia: vi.fn(),
    hold: () => {
      let release: () => void = () => undefined;
      media.getUserMedia.mockImplementationOnce(
        () =>
          new Promise<FakeStream>((resolve) => {
            release = () => {
              const stream = new FakeStream();
              media.streams.push(stream);
              resolve(stream);
            };
          }),
      );
      return { answer: () => release() };
    },
    refuse: (name) => {
      media.getUserMedia.mockImplementation(() => Promise.reject(new DOMException("refused", name)));
    },
  };
  media.getUserMedia.mockImplementation(() => {
    const stream = new FakeStream();
    media.streams.push(stream);
    return Promise.resolve(stream);
  });
  Object.defineProperty(navigator, "mediaDevices", { value: { getUserMedia: media.getUserMedia }, configurable: true });
  vi.stubGlobal("MediaRecorder", FakeRecorder);
  return media;
}

export function uninstallFakeMedia() {
  vi.unstubAllGlobals();
  Reflect.deleteProperty(navigator, "mediaDevices");
}
