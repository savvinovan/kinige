import type { TFunction } from "i18next";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import { WARN_FROM_MS } from "../../voice/recorderSession";
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
 */
export function RecordingBar({ state, controls }: RecordingBarProps) {
  const { t } = useTranslation();
  const sendButton = useRef<HTMLSpanElement>(null);
  const keepButton = useRef<HTMLButtonElement>(null);
  const confirming = state.kind === "confirming";

  // Focus moves to Send when the bar appears, and to "Keep recording" while it asks: Enter then never discards by accident.
  useEffect(() => {
    if (confirming) keepButton.current?.focus();
    else sendButton.current?.querySelector("button")?.focus();
  }, [confirming]);

  // Escape works wherever the focus is (the person may have clicked into the feed): cancel, or "keep" while the question is open.
  useEffect(() => {
    if (state.kind !== "recording" && state.kind !== "confirming" && state.kind !== "lost" && state.kind !== "send-failed") return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      event.preventDefault();
      if (state.kind === "confirming") controls.keepRecording();
      else if (state.kind === "recording") controls.requestCancel();
      else controls.discard();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [state.kind, controls]);

  if (state.kind === "confirming") {
    return (
      <div className="recording-bar recording-bar--confirm" role="alertdialog" aria-label={t("voice.discardTitle")}>
        <p className="recording-bar__question">{t("voice.discardTitle")}</p>
        <button type="button" className="recording-bar__choice" onClick={controls.discard}>
          {t("voice.discard")}
        </button>
        <button type="button" className="recording-bar__choice recording-bar__choice--keep" ref={keepButton} onClick={controls.keepRecording}>
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

  const trash = state.kind === "recording" ? controls.requestCancel : controls.discard;
  return (
    <div
      className={`recording-bar${sending ? " recording-bar--sending" : ""}`}
      role="group"
      aria-label={t("voice.recording", { time })}
    >
      <IconButton icon="trash" label={t("voice.discardRecording")} onClick={trash} disabled={sending} />
      {state.kind === "lost" ? (
        <p className="recording-bar__lost" role="status">
          {t("voice.disconnected", { time })}
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

function failureText(t: TFunction, code: ErrorCode): string {
  return `${t("voice.sendFailed")} ${t(`error.${code}`)}`;
}
