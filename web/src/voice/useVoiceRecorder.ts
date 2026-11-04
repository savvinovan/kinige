import { useCallback, useEffect, useRef, useState } from "react";
import { type KinigeApi, errorCode } from "../api/KinigeApi";
import type { ErrorCode } from "../api/generated/ErrorCode";
import type { MessageView } from "../api/generated/MessageView";
import {
  CONFIRM_AFTER_MS,
  MAX_RECORDING_MS,
  MIN_RECORDING_MS,
  type MicError,
  type Recording,
  RecorderSession,
  micErrorOf,
} from "./recorderSession";

/** How often the timer and the meter redraw. */
const TICK_MS = 250;

/** What the composer shows for the microphone. `idle` is the plain microphone button. */
export type VoiceState =
  | { kind: "idle" }
  /** Waiting for the system's answer to the first request; the button is busy. */
  | { kind: "starting" }
  | { kind: "recording"; elapsedMs: number; levels: readonly number[] }
  /** "Discard the recording?": the recording goes on underneath. */
  | { kind: "confirming"; elapsedMs: number; levels: readonly number[] }
  /** The device was unplugged: what was recorded is kept, Send and Discard are offered. */
  | { kind: "lost"; elapsedMs: number }
  | { kind: "sending"; elapsedMs: number }
  /** Saving failed (a full disk): the bytes stay in memory; Retry and Discard. */
  | { kind: "send-failed"; elapsedMs: number; error: ErrorCode }
  /** One line above the field: access off, no microphone, busy, too short. */
  | { kind: "notice"; reason: MicError | "too-short" };

export interface VoiceControls {
  state: VoiceState;
  /** The first click on the microphone: asks the system for it and starts. */
  start: () => void;
  /** Send, Enter and the auto-send. */
  send: () => void;
  /** The trash button and Escape: under 5 seconds drops at once, longer asks first. */
  requestCancel: () => void;
  /** "Keep recording" in the confirmation. */
  keepRecording: () => void;
  /** "Discard" in the confirmation, after a lost device or a failed save. */
  discard: () => void;
  /** "Retry" after a failed save. */
  retry: () => void;
  /** "Open settings" on the access-off line. */
  openSettings: () => void;
  /** Hides the line above the field. */
  dismissNotice: () => void;
}

interface UseVoiceRecorderOptions {
  api: KinigeApi;
  /** The open chat; `null` means the trash is open and nothing is recorded. */
  chatId: string | null;
  /** The entry the draft replies to (live replies only); the note carries it like a text reply. */
  replyToId: string | null;
  /** A note was saved in `chatId` (the chat it was started in). */
  onSent: (sent: MessageView, chatId: string) => void;
}

/**
 * The microphone of the composer (slice 7a): start on click, finish with Send or Enter, cancel with the trash button or Escape (a recording
 * over five seconds asks first), keep what was recorded when the device is lost, and send by itself at 30 minutes or 48 MiB. The note goes
 * to the chat it was started in even if the person has opened another one since. A recording still running when the window closes is
 * lost, as in Telegram.
 */
export function useVoiceRecorder({ api, chatId, replyToId, onSent }: UseVoiceRecorderOptions): VoiceControls {
  const [state, setState] = useState<VoiceState>({ kind: "idle" });
  const session = useRef<RecorderSession | null>(null);
  /** What the recording was started for: its own chat and reply. */
  const target = useRef<{ chatId: string; replyToId: string | null } | null>(null);
  /** The recording kept after a lost device or a failed save, waiting for Send / Retry / Discard. */
  const kept = useRef<Recording | null>(null);
  const confirming = useRef(false);
  const busy = useRef(false);
  /** The latest `send`: the byte limit and the 30 minute timer call it from outside React's render. */
  const sendRef = useRef<() => void>(() => undefined);
  const latest = useRef({ chatId, replyToId, onSent });
  useEffect(() => {
    latest.current = { chatId, replyToId, onSent };
  });

  const clear = useCallback(() => {
    session.current = null;
    target.current = null;
    kept.current = null;
    confirming.current = false;
    busy.current = false;
    setState({ kind: "idle" });
  }, []);

  /** Saves a finished recording in the chat it was started in. */
  const save = useCallback(
    async (recording: Recording) => {
      const where = target.current;
      if (!where) return;
      setState({ kind: "sending", elapsedMs: recording.durationMs });
      try {
        const sent = await api.sendVoice(where.chatId, recording.bytes, recording.durationMs, where.replyToId);
        latest.current.onSent(sent, where.chatId);
        clear();
      } catch (reason) {
        const code = errorCode(reason);
        // Too short and too long can never succeed on a retry: they end the recording. Anything else keeps the bytes.
        if (code === "voice_too_short") {
          clear();
          setState({ kind: "notice", reason: "too-short" });
          return;
        }
        kept.current = recording;
        busy.current = false;
        setState({ kind: "send-failed", elapsedMs: recording.durationMs, error: code });
      }
    },
    [api, clear],
  );

  const send = useCallback(() => {
    if (busy.current) return;
    busy.current = true;
    const running = session.current;
    if (running) {
      session.current = null;
      running
        .finish()
        .then((recording) => {
          if (recording.durationMs < MIN_RECORDING_MS) {
            clear();
            setState({ kind: "notice", reason: "too-short" });
            return undefined;
          }
          return save(recording);
        })
        // Reading the chunks failed: nothing can be saved, the microphone is already released.
        .catch(() => {
          clear();
          setState({ kind: "notice", reason: "busy" });
        });
      return;
    }
    const recording = kept.current;
    if (recording) void save(recording);
    else busy.current = false;
  }, [save, clear]);

  const start = useCallback(() => {
    const where = latest.current.chatId;
    if (where === null || session.current || busy.current || kept.current) return;
    busy.current = true;
    target.current = { chatId: where, replyToId: latest.current.replyToId };
    setState({ kind: "starting" });
    RecorderSession.start({
      onDeviceLost: () => {
        // The recorder has stopped by itself: keep the bytes and offer Send / Discard.
        const lost = session.current;
        if (!lost) return;
        session.current = null;
        confirming.current = false;
        void lost.finish().then((recording) => {
          kept.current = recording;
          setState({ kind: "lost", elapsedMs: recording.durationMs });
        });
      },
      onByteLimit: () => sendRef.current(),
    }).then(
      (started) => {
        session.current = started;
        busy.current = false;
        setState({ kind: "recording", elapsedMs: 0, levels: started.levelBars() });
      },
      (reason: unknown) => {
        clear();
        setState({ kind: "notice", reason: micErrorOf(reason) });
      },
    );
  }, [clear]);

  useEffect(() => {
    sendRef.current = send;
  });

  const requestCancel = useCallback(() => {
    const running = session.current;
    if (!running) return;
    if (running.elapsedMs() < CONFIRM_AFTER_MS) {
      session.current = null;
      running.abort();
      clear();
      return;
    }
    confirming.current = true;
    setState({ kind: "confirming", elapsedMs: running.elapsedMs(), levels: running.levelBars() });
  }, [clear]);

  const keepRecording = useCallback(() => {
    const running = session.current;
    if (!running) return;
    confirming.current = false;
    setState({ kind: "recording", elapsedMs: running.elapsedMs(), levels: running.levelBars() });
  }, []);

  const discard = useCallback(() => {
    const running = session.current;
    session.current = null;
    running?.abort();
    clear();
  }, [clear]);

  const retry = useCallback(() => {
    const recording = kept.current;
    if (!recording || busy.current) return;
    busy.current = true;
    void save(recording);
  }, [save]);

  const openSettings = useCallback(() => {
    void api.openMicrophoneSettings().catch(() => undefined);
  }, [api]);

  const dismissNotice = useCallback(() => setState((was) => (was.kind === "notice" ? { kind: "idle" } : was)), []);

  // The timer and the meter follow the session; at 30 minutes the note is sent by itself.
  useEffect(() => {
    if (state.kind !== "recording" && state.kind !== "confirming") return;
    const timer = setInterval(() => {
      const running = session.current;
      if (!running) return;
      const elapsedMs = running.elapsedMs();
      if (elapsedMs >= MAX_RECORDING_MS) {
        sendRef.current();
        return;
      }
      const levels = running.levelBars();
      setState(confirming.current ? { kind: "confirming", elapsedMs, levels } : { kind: "recording", elapsedMs, levels });
    }, TICK_MS);
    return () => clearInterval(timer);
  }, [state.kind]);

  // Closing the window (or the whole main window unmounting) loses a running recording and releases the microphone.
  useEffect(
    () => () => {
      session.current?.abort();
      session.current = null;
    },
    [],
  );

  // A notice goes away by itself once something else happens: a new start replaces it, Escape and the cross dismiss it.
  return { state, start, send, requestCancel, keepRecording, discard, retry, openSettings, dismissNotice };
}
