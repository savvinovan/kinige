import { useTranslation } from "react-i18next";
import type { VoiceState } from "../../voice/useVoiceRecorder";
import "./recording-bar.css";

interface VoiceNoticeProps {
  state: Extract<VoiceState, { kind: "notice" | "starting" }>;
  onOpenSettings: () => void;
  onDismiss: () => void;
}

/**
 * The line above the field when the microphone cannot be used (slice 7a): "Microphone access is off" with "Open settings", "No microphone
 * found", "The microphone is unavailable", "Too short to save". No dialog and no toast: the same place as the other composer errors.
 */
export function VoiceNotice({ state, onOpenSettings, onDismiss }: VoiceNoticeProps) {
  const { t } = useTranslation();
  if (state.kind === "starting") {
    return (
      <p className="voice-notice" role="status">
        {t("voice.starting")}
      </p>
    );
  }
  const text = {
    denied: t("voice.denied"),
    "no-device": t("voice.noDevice"),
    busy: t("voice.busy"),
    "too-short": t("voice.tooShort"),
    "too-long": t("voice.tooLong"),
  }[state.reason];
  return (
    <p className="voice-notice voice-notice--error" role="alert">
      <span>{text}</span>
      {state.reason === "denied" ? (
        <button type="button" className="voice-notice__action" onClick={onOpenSettings}>
          {t("voice.openSettings")}
        </button>
      ) : null}
      <button type="button" className="voice-notice__action" onClick={onDismiss}>
        {t("voice.dismiss")}
      </button>
    </p>
  );
}
