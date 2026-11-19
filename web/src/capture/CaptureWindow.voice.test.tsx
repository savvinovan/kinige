import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../api/KinigeApi";
import { MockApi } from "../api/MockApi";
import { type FakeMedia, installFakeMedia, uninstallFakeMedia } from "../test/fakeMedia";
import { act, fireEvent, render, screen } from "../test/render";
import { CaptureWindow, HIDE_REQUEST_EVENT, SHOWN_EVENT } from "./CaptureWindow";

// Voice notes from the quick capture window (KINIG-153): the main window's microphone button and recording bar in the capture panel, what Esc,
// the hotkey (a hide request from Rust) and a new showing do while it records, and the single microphone shared with the main window.

const NOW = new Date(Date.UTC(2026, 9, 21, 6, 12));
let media: FakeMedia;

beforeEach(() => {
  vi.useFakeTimers();
  media = installFakeMedia();
});

afterEach(() => {
  vi.useRealTimers();
  uninstallFakeMedia();
});

async function setup() {
  const api = new MockApi({ now: () => NOW, language: "en" });
  const initial = await api.captureState();
  const view = render(<CaptureWindow api={api} initial={initial} />, { language: "en" });
  return { api, ...view };
}

async function settle(ms = 20) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

const mic = () => screen.getByRole("button", { name: "Record voice note" });
const escape = () => fireEvent.keyDown(document.body, { key: "Escape" });

async function startRecording() {
  fireEvent.click(mic());
  await settle();
  expect(screen.getByRole("group", { name: /^Recording, / })).toBeInTheDocument();
}

describe("the microphone in the capture window", () => {
  it("is the main window's button in the field's right slot, and gives way to Send while there is text", async () => {
    await setup();
    expect(mic()).toBeInTheDocument();
    expect(mic()).not.toHaveAttribute("aria-disabled");
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "a note" } });
    expect(screen.queryByRole("button", { name: "Record voice note" })).toBeNull();
    expect(screen.getByRole("button", { name: "Send" })).toBeInTheDocument();
  });

  it("records, sends to the Inbox and hides like after a text note; the note is in the window's list when it comes back", async () => {
    const { api } = await setup();
    await startRecording();
    await settle(6000);
    fireEvent.click(screen.getByRole("button", { name: "Send the voice note" }));
    await settle();
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]).toMatchObject({ chatId: INBOX_ID, replyToId: null });
    expect(api.hideCount).toBe(1);
    expect(api.microphoneHolder).toBeNull();
    expect(media.streams[0]?.track.stopped).toBe(true);
    // Shown again: a fresh state with the note in the list, as the others are.
    await act(async () => {
      window.dispatchEvent(new Event(SHOWN_EVENT));
      await vi.advanceTimersByTimeAsync(20);
    });
    const state = await api.captureState();
    expect(state.context.at(-1)?.attachments?.[0]?.kind).toBe("audio");
    expect(screen.getByRole("list", { name: "Latest entries today" })).toHaveTextContent("Voice note");
    expect(mic()).toBeInTheDocument();
  });

  it("a refused permission says so with 'Open settings' that opens the page of the capture window's own command", async () => {
    const { api } = await setup();
    media.refuse("NotAllowedError");
    fireEvent.click(mic());
    await settle();
    expect(screen.getByText("Microphone access is off")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    await settle();
    expect(api.microphoneSettingsOpened).toBe(1);
    expect(api.hideCount).toBe(0);
  });

  it("while the main window records, the button is answered with 'unavailable' and no second recording starts", async () => {
    const { api } = await setup();
    await api.holdMicrophone(true);
    fireEvent.click(mic());
    await settle();
    expect(screen.getByText("The microphone is unavailable")).toBeInTheDocument();
    expect(media.getUserMedia).not.toHaveBeenCalled();
    expect(api.microphoneHolder).toBe("main");
  });
});

describe("Esc", () => {
  it("hides the window when nothing is recording, as before", async () => {
    const { api } = await setup();
    escape();
    await settle();
    expect(api.hideCount).toBe(1);
  });

  it("belongs to the recording bar while it stands in for the field: a short recording is dropped and the window stays, the next Esc hides", async () => {
    const { api } = await setup();
    await startRecording();
    await settle(2000);
    escape();
    await settle();
    expect(screen.queryByRole("group", { name: /^Recording, / })).toBeNull();
    expect(api.hideCount).toBe(0);
    expect(api.microphoneHolder).toBeNull();
    expect(media.streams[0]?.track.stopped).toBe(true);
    expect(mic()).toBeInTheDocument();
    escape();
    await settle();
    expect(api.hideCount).toBe(1);
  });

  it("asks before dropping a long recording, and the next Esc keeps it; the window stays all along", async () => {
    const { api } = await setup();
    await startRecording();
    await settle(8000);
    escape();
    await settle();
    expect(screen.getByRole("alertdialog", { name: "Discard the recording?" })).toBeInTheDocument();
    expect(api.hideCount).toBe(0);
    escape();
    await settle();
    expect(screen.getByRole("group", { name: /^Recording, / })).toBeInTheDocument();
    expect(api.hideCount).toBe(0);
    expect(api.microphoneHolder).toBe("capture");
  });
});

describe("the window is hidden while it records (its hotkey, closing, or Rust giving up waiting)", () => {
  async function hideRequest() {
    await act(async () => {
      window.dispatchEvent(new Event(HIDE_REQUEST_EVENT));
      await vi.advanceTimersByTimeAsync(50);
    });
  }

  it("stops the recording, keeps it, hides, and offers Send or Discard the next time the window opens", async () => {
    const { api } = await setup();
    await startRecording();
    await settle(8000);
    await hideRequest();
    expect(api.hideCount).toBe(1);
    expect(api.microphoneHolder).toBeNull();
    expect(media.streams[0]?.track.stopped).toBe(true);
    expect(api.sentVoices).toHaveLength(0);
    // Back again: the kept recording is there, in words that do not blame the device.
    await act(async () => {
      window.dispatchEvent(new Event(SHOWN_EVENT));
      await vi.advanceTimersByTimeAsync(20);
    });
    expect(screen.getByRole("status")).toHaveTextContent("Recording stopped · 0:08");
    expect(screen.queryByText(/disconnected/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Send the voice note" }));
    await settle();
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]?.chatId).toBe(INBOX_ID);
    expect(api.hideCount).toBe(2);
  });

  /** Records for `ms`, has the window hidden by the hotkey's request, and shows it again: the kept recording is on screen. */
  async function keptAfterReopen(ms: number) {
    const opened = await setup();
    await startRecording();
    await settle(ms);
    await hideRequest();
    await act(async () => {
      window.dispatchEvent(new Event(SHOWN_EVENT));
      // Rust gives the reopened window the key: the page puts the focus where the bar wants it.
      window.dispatchEvent(new Event("focus"));
      await vi.advanceTimersByTimeAsync(20);
    });
    expect(screen.getByRole("status")).toHaveTextContent("Recording stopped");
    return opened;
  }

  it("Esc on a reopened window hides it and keeps the recording: the habitual key loses nothing", async () => {
    const { api } = await keptAfterReopen(8000);
    expect(api.hideCount).toBe(1);
    escape();
    await settle();
    expect(api.hideCount).toBe(2);
    expect(api.sentVoices).toHaveLength(0);
    expect(screen.getByRole("status")).toHaveTextContent("Recording stopped · 0:08");
    // And again: it is still there, Esc after Esc.
    escape();
    await settle();
    expect(api.hideCount).toBe(3);
    expect(screen.getByRole("status")).toHaveTextContent("Recording stopped · 0:08");
  });

  it("the reopened window puts the focus on the bar and on no button, so Space or Enter send and delete nothing", async () => {
    const { api } = await keptAfterReopen(8000);
    const focused = document.activeElement as HTMLElement;
    expect(focused.tagName).not.toBe("BUTTON");
    expect(focused).toHaveAttribute("data-kept", "hidden");
    expect(focused).toHaveAttribute("role", "group");
    // jsdom does not turn a key on a button into a click, so the focus is the proof; a key on the bar itself does nothing.
    for (const key of ["Enter", " "]) {
      fireEvent.keyDown(focused, { key });
      fireEvent.keyUp(focused, { key });
    }
    await settle();
    expect(api.sentVoices).toHaveLength(0);
    expect(screen.getByRole("status")).toHaveTextContent("Recording stopped");
    // The buttons are still there for Tab and the pointer.
    expect(screen.getByRole("button", { name: "Send the voice note" })).not.toHaveFocus();
    expect(screen.getByRole("button", { name: "Discard the recording" })).not.toHaveFocus();
  });

  it("the trash button of a kept recording over five seconds asks first; Keep leaves it, Discard drops it", async () => {
    const { api } = await keptAfterReopen(8000);
    fireEvent.click(screen.getByRole("button", { name: "Discard the recording" }));
    await settle();
    expect(screen.getByRole("alertdialog", { name: "Discard the recording?" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Keep recording" })).toHaveFocus();
    // Esc while it asks hides the window as everywhere else; the question and the recording are still there.
    escape();
    await settle();
    expect(api.hideCount).toBe(2);
    expect(screen.getByRole("alertdialog", { name: "Discard the recording?" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Keep recording" }));
    await settle();
    expect(screen.getByRole("status")).toHaveTextContent("Recording stopped · 0:08");
    fireEvent.click(screen.getByRole("button", { name: "Discard the recording" }));
    await settle();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    await settle();
    expect(mic()).toBeInTheDocument();
    expect(api.sentVoices).toHaveLength(0);
  });

  it("a kept recording under five seconds is dropped by the trash button at once", async () => {
    const { api } = await keptAfterReopen(3000);
    fireEvent.click(screen.getByRole("button", { name: "Discard the recording" }));
    await settle();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(mic()).toBeInTheDocument();
    expect(api.sentVoices).toHaveLength(0);
  });

  it("Send on a kept recording still sends it to the Inbox and hides the window", async () => {
    const { api } = await keptAfterReopen(8000);
    fireEvent.click(screen.getByRole("button", { name: "Send the voice note" }));
    await settle();
    expect(api.sentVoices).toHaveLength(1);
    expect(api.sentVoices[0]?.chatId).toBe(INBOX_ID);
    expect(api.hideCount).toBe(2);
  });

  it("while the system's permission prompt is still up nothing is recorded behind the hidden window", async () => {
    const { api } = await setup();
    const prompt = media.hold();
    fireEvent.click(mic());
    await settle();
    expect(screen.getByRole("status")).toHaveTextContent("Waiting for the microphone");
    await hideRequest();
    expect(api.hideCount).toBe(1);
    await act(async () => {
      prompt.answer();
      await vi.advanceTimersByTimeAsync(20);
    });
    expect(screen.queryByRole("group", { name: /^Recording, / })).toBeNull();
    expect(media.streams[0]?.track.stopped).toBe(true);
    expect(api.microphoneHolder).toBeNull();
    expect(api.sentVoices).toHaveLength(0);
  });
});

describe("keyboard focus", () => {
  it("goes to Send when the window gets the focus back while recording, not to the field that is not there", async () => {
    await setup();
    await startRecording();
    (document.activeElement as HTMLElement | null)?.blur();
    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(screen.getByRole("button", { name: "Send the voice note" })).toHaveFocus();
  });
});
