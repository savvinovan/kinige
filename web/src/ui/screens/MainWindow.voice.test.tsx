import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import { act, fireEvent, render, screen, waitFor } from "../../test/render";
import { MainWindow } from "./MainWindow";

// The microphone on the real `MainWindow` and `MockApi` (slice 7a), with a fake `MediaDevices` and `MediaRecorder`: the click, the bar,
// Send, the note in the feed, the pending tiles that hide the microphone, and a note sent from another chat.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

class FakeTrack extends EventTarget {
  stopped = false;
  stop() {
    this.stopped = true;
  }
}

class FakeRecorder extends EventTarget {
  state: "inactive" | "recording" = "inactive";
  static isTypeSupported() {
    return true;
  }
  start() {
    this.state = "recording";
  }
  stop() {
    this.state = "inactive";
    const event = new Event("dataavailable") as Event & { data: Blob };
    event.data = new Blob([new Uint8Array(2048)]);
    this.dispatchEvent(event);
    this.dispatchEvent(new Event("stop"));
  }
}

let track: FakeTrack;

beforeEach(() => {
  track = new FakeTrack();
  const stream = { getAudioTracks: () => [track], getTracks: () => [track] };
  Object.defineProperty(navigator, "mediaDevices", { value: { getUserMedia: vi.fn(() => Promise.resolve(stream)) }, configurable: true });
  vi.stubGlobal("MediaRecorder", FakeRecorder);
  // jsdom has no media engine and says "" for every type, which would make every player "Can't play here".
  vi.spyOn(HTMLMediaElement.prototype, "canPlayType").mockReturnValue("maybe");
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(navigator, "mediaDevices");
});

function setup() {
  const api = new MockApi({ now: NOW, language: "en", chats: [{ id: "chat-2", title: "Books", icon: "📚" }] });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "en" });
  return api;
}

const mic = () => screen.findByRole("button", { name: "Record voice note" });

describe("MainWindow: voice notes", () => {
  it("a click on the microphone shows the recording bar, Send saves the note and it appears in the feed", async () => {
    const api = setup();
    fireEvent.click(await mic());
    expect(await screen.findByRole("group", { name: /^Recording, / })).toBeInTheDocument();
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 1100));
    });
    fireEvent.click(screen.getByRole("button", { name: "Send the voice note" }));
    await waitFor(() => expect(api.sentVoices).toHaveLength(1));
    expect(api.sentVoices[0]).toMatchObject({ chatId: INBOX_ID });
    expect(await screen.findByRole("group", { name: /^Voice note, \d+ seconds?$/ })).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: /^Recording, / })).toBeNull();
    expect(track.stopped).toBe(true);
    // The microphone is back on an empty field.
    expect(await mic()).toBeInTheDocument();
  });

  it("with a pending tile the microphone is replaced by Send, so a note never goes beside photos", async () => {
    const api = setup();
    api.pickedFiles = ["a.jpg"];
    fireEvent.click(await screen.findByRole("button", { name: "Attach file" }));
    expect(await screen.findByRole("button", { name: "Remove a.jpg" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Record voice note" })).toBeNull();
  });

  it("without a microphone the line above the field says so and nothing else happens", async () => {
    Reflect.deleteProperty(navigator, "mediaDevices");
    setup();
    fireEvent.click(await mic());
    expect(await screen.findByText("No microphone found")).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: /^Recording, / })).toBeNull();
  });
});
