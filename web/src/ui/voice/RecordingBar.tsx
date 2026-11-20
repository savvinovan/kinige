import type { TFunction } from "i18next";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import { CONFIRM_AFTER_MS, WARN_FROM_MS } from "../../voice/recorderSession";
import { formatDuration } from "../../voice/format";
import type { VoiceControls, VoiceState } from "../../voice/useVoiceRecorder";
import { IconButton } from "../controls/IconButton";
import { LiveWave } from "./LiveWave";
import "./recording-bar.css";

type BarState = Extract<VoiceState, { kind: "recording" | "confirming" | "lost" | "sending" | "send-failed" }>;

interface RecordingBarProps {
  state: BarState;
  controls: Pick<VoiceControls, "send" | "requestCancel" | "keepRecording" | "discard" | "retry" | "levels">;
}

/**
 * The bar that replaces the input field while recording (slice 7a): the trash button on the left, a red dot that does not pulse, the timer in
 * tabular figures (the warning colour from 29:00), the live waveform across the whole middle of the field (new bars enter from the right and
 * scroll left; see `LiveWave`) and the round accent Send on the right. It keeps the height of the field. Focus goes to Send; Enter sends,
 * Escape cancels (a recording over five seconds asks first).
 *
 * A recording kept because the capture window was hidden (`lost` with `cause: "hidden"`, KINIG-153) is different: it is there when the window
 * comes back, where the habitual keys are Esc, Space and Enter, so none of them may send or delete it. Esc is left to the window (it hides
 * and keeps the recording), the focus goes to the bar itself and not to a button, and the trash button asks first over five seconds, as it
 * does for a recording that is running.
 */
export function RecordingBar({ state, controls }: RecordingBarProps) {
  const { t } = useTranslation();
  const bar = useRef<HTMLDivElement>(null);
  const sendButton = useRef<HTMLSpanElement>(null);
  const keepButton = useRef<HTMLButtonElement>(null);
  const kept = state.kind === "lost" && state.cause === "hidden";
  const [asking, setAsking] = useState(false);
  // The question belongs to one kept recording: when there is none, it is forgotten (state adjusted while rendering, not in an effect).
  if (!kept && asking) setAsking(false);
  const question = state.kind === "confirming" || (kept && asking);

  // Focus moves to Send when the bar appears, and to "Keep recording" while it asks: Enter then never discards by accident. A kept recording
  // of the capture window takes it on the bar itself: no single key sends or deletes it, Tab reaches Discard and Send.
  useEffect(() => {
    if (question) keepButton.current?.focus();
    else if (kept) bar.current?.focus();
    else sendButton.current?.querySelector("button")?.focus();
  }, [question, kept]);

  // Escape works wherever the focus is (the person may have clicked into the feed): cancel, or "keep" while the question is open.
  useEffect(() => {
    if (!barOwnsEscape(state)) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      event.preventDefault();
      if (state.kind === "confirming") controls.keepRecording();
      else if (state.kind === "recording") controls.requestCancel();
      else controls.discard();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [state, controls]);

  if (question) {
    return (
      <div className="recording-bar recording-bar--confirm" role="alertdialog" aria-label={t("voice.discardTitle")}>
        <p className="recording-bar__question">{t("voice.discardTitle")}</p>
        <button type="button" className="recording-bar__choice" onClick={controls.discard}>
          {t("voice.discard")}
        </button>
        <button
          type="button"
          className="recording-bar__choice recording-bar__choice--keep"
          ref={keepButton}
          onClick={state.kind === "confirming" ? controls.keepRecording : () => setAsking(false)}
        >
          {t("voice.keep")}
        </button>
      </div>
    );
  }

  const time = formatDuration(state.elapsedMs);
  const sending = state.kind === "sending";

  if (state.kind === "send-failed") {
    return (
      <div className="recording-bar recording-bar--failed" role="alert">
        <p className="recording-bar__question">{failureText(t, state.error)}</p>
        <button type="button" className="recording-bar__choice" onClick={controls.discard}>
          {t("voice.discard")}
        </button>
        {state.retryable ? (
          <button type="button" className="recording-bar__choice recording-bar__choice--keep" onClick={controls.retry}>
            {t("voice.retry")}
          </button>
        ) : null}
      </div>
    );
  }

  const askFirst = kept && state.elapsedMs >= CONFIRM_AFTER_MS;
  const trash = state.kind === "recording" ? controls.requestCancel : askFirst ? () => setAsking(true) : controls.discard;
  return (
    <div
      className={`recording-bar${sending ? " recording-bar--sending" : ""}`}
      role="group"
      aria-label={t("voice.recording", { time })}
      ref={bar}
      tabIndex={kept ? -1 : undefined}
      data-kept={kept ? "hidden" : undefined}
    >
      <IconButton icon="trash" label={t("voice.discardRecording")} onClick={trash} disabled={sending} />
      {state.kind === "lost" ? (
        <p className="recording-bar__lost" role="status">
          {t(state.cause === "hidden" ? "voice.stopped" : "voice.disconnected", { time })}
        </p>
      ) : (
        <>
          <span className="recording-bar__dot" aria-hidden="true" />
          <span
            className={`recording-bar__timer${state.kind === "recording" && state.elapsedMs >= WARN_FROM_MS ? " recording-bar__timer--warn" : ""}`}
          >
            {time}
          </span>
          <LiveWave read={controls.levels} label={t("voice.level")} />
        </>
      )}
      <span className="composer__action" ref={sendButton}>
        <IconButton icon="send" label={t("voice.send")} tone="accent" onClick={controls.send} disabled={sending} />
      </span>
    </div>
  );
}

/** The states in which the bar answers Escape. A window with its own Escape (the capture window hides on it) leaves it to the bar then. */
export function barOwnsEscape(state: VoiceState): boolean {
  if (state.kind === "lost") return state.cause !== "hidden";
  return state.kind === "recording" || state.kind === "confirming" || state.kind === "send-failed";
}

function failureText(t: TFunction, code: ErrorCode): string {
  return `${t("voice.sendFailed")} ${t(`error.${code}`)}`;
}
