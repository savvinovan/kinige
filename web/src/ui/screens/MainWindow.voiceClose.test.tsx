import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MockApi } from "../../api/MockApi";
import { DEFAULT_SETTINGS } from "../../api/MockApi";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Closing the main window while recording (KINIG-156, ADR-0015 round 5): Rust sends `main-window-hiding` before it hides the window; the page stops the
// recording, lets go of the tracks and the microphone claim, and keeps the bytes as "Recording stopped" for when the window comes back.
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
  vi.spyOn(HTMLMediaElement.prototype, "canPlayType").mockReturnValue("maybe");
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(navigator, "mediaDevices");
});

function mount() {
  const api = new MockApi({ now: NOW, language: "en", chats: [{ id: "chat-2", title: "Books", icon: "📚" }] });
  const view = render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "en" });
  return { api, unmount: view.unmount };
}

function setup() {
  return mount().api;
}

const mic = () => screen.findByRole("button", { name: "Record voice note" });
const recording = () => screen.findByRole("group", { name: /^Recording, / });
const wait = (ms: number) =>
  act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
const closeWindow = (api: MockApi) => act(async () => api.emitMainWindowHiding());
const claimCalls = (api: MockApi) => api.calls.filter((call) => call.startsWith("holdMicrophone"));

describe("closing the main window while recording", () => {
  it("stops the recording, lets go of the tracks and the claim, and keeps the bytes as 'Recording stopped'", async () => {
    const api = setup();
    fireEvent.click(await mic());
    await recording();
    await wait(1100);
    expect(track.stopped).toBe(false);

    await closeWindow(api);

    const kept = await screen.findByRole("group", { name: /^Recording, 0:0[1-9]$/ });
    expect(within(kept).getByText(/^Recording stopped · 0:0[1-9]$/)).toBeInTheDocument();
    expect(track.stopped).toBe(true);
    expect(claimCalls(api).at(-1)).toBe("holdMicrophone false");
    expect(api.sentVoices).toHaveLength(0);
    // The kept recording is the whole bar: Send and Discard, no timer or waveform of a running one.
    expect(within(kept).getByRole("button", { name: "Send the voice note" })).toBeInTheDocument();
    expect(within(kept).getByRole("button", { name: "Discard the recording" })).toBeInTheDocument();
  });

  it("puts the focus on the bar, so Space and Enter send nothing, and Send then saves the note in the chat it was started in", async () => {
    const api = setup();
    fireEvent.click(await screen.findByRole("button", { name: /^Books/ }));
    await waitFor(() => expect(screen.getByRole("textbox", { name: "New note" })).toBeInTheDocument());
    fireEvent.click(await mic());
    await recording();
    await wait(1100);
    // The person has moved on to the Inbox by the time the window is closed: the note still belongs to Books.
    fireEvent.click(screen.getByRole("button", { name: /^Inbox/ }));
    await closeWindow(api);

    const kept = await screen.findByRole("group", { name: /^Recording, / });
    await waitFor(() => expect(kept).toHaveFocus());
    for (const key of [" ", "Enter"]) {
      fireEvent.keyDown(kept, { key });
      fireEvent.keyUp(kept, { key });
    }
    expect(api.sentVoices).toHaveLength(0);

    fireEvent.click(within(kept).getByRole("button", { name: "Send the voice note" }));
    await waitFor(() => expect(api.sentVoices).toHaveLength(1));
    expect(api.sentVoices[0]).toMatchObject({ chatId: "chat-2" });
  });

  it("under a second there is nothing to keep: the line says 'Too short to save'", async () => {
    const api = setup();
    fireEvent.click(await mic());
    await recording();
    await closeWindow(api);
    expect(await screen.findByText("Too short to save")).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: /^Recording, / })).toBeNull();
    expect(track.stopped).toBe(true);
    expect(claimCalls(api).at(-1)).toBe("holdMicrophone false");
    expect(api.sentVoices).toHaveLength(0);
  });

  it("while the system is still answering the first request, the start is dropped and nothing is recorded", async () => {
    const api = setup();
    let answer: (stream: unknown) => void = () => undefined;
    const pending = new Promise((resolve) => (answer = resolve));
    Object.defineProperty(navigator, "mediaDevices", { value: { getUserMedia: vi.fn(() => pending) }, configurable: true });
    fireEvent.click(await mic());
    await waitFor(() => expect(screen.getByRole("button", { name: "Record voice note" })).toHaveAttribute("aria-busy", "true"));

    await closeWindow(api);
    await act(async () => answer({ getAudioTracks: () => [track], getTracks: () => [track] }));

    await waitFor(() => expect(track.stopped).toBe(true));
    expect(screen.queryByRole("group", { name: /^Recording/ })).toBeNull();
    expect(await mic()).toBeInTheDocument();
    expect(api.sentVoices).toHaveLength(0);
  });

  it("'Discard the recording?' open at the close becomes the kept recording", async () => {
    const api = setup();
    fireEvent.click(await mic());
    await recording();
    await wait(5200);
    fireEvent.click(screen.getByRole("button", { name: "Discard the recording" }));
    expect(await screen.findByRole("alertdialog", { name: "Discard the recording?" })).toBeInTheDocument();

    await closeWindow(api);

    expect(await screen.findByText(/^Recording stopped · 0:0[5-9]$/)).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(track.stopped).toBe(true);
    // Over five seconds, so the trash button asks before it deletes what was kept.
    fireEvent.click(screen.getByRole("button", { name: "Discard the recording" }));
    expect(await screen.findByRole("alertdialog", { name: "Discard the recording?" })).toBeInTheDocument();
  }, 15_000);

  it("nothing happens when nothing is recording", async () => {
    const api = setup();
    await mic();
    await closeWindow(api);
    expect(await mic()).toBeInTheDocument();
    expect(claimCalls(api)).toEqual([]);
  });

  it("a close after the window's page is gone is ignored: the listener was removed, nothing stops or throws", async () => {
    const { api, unmount } = mount();
    fireEvent.click(await mic());
    await recording();
    await wait(1100);
    unmount();
    // The page's own cleanup lets go of the tracks and the claim; the close that comes after it finds no listener and does no further work.
    const callsBefore = api.calls.length;
    await closeWindow(api);
    expect(api.calls.length).toBe(callsBefore);
  });
});
