import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import type { ShortcutStatus } from "../../api/generated/ShortcutStatus";
import { Button } from "../controls/Button";
import { acceleratorFromEvent, formatAccelerator, parseAccelerator, type Modifier } from "./accelerator";
import { StatusNote } from "./StatusNote";
import "./shortcut-row.css";

interface ShortcutRowProps {
  status: ShortcutStatus;
  /** The already translated save error text; shown under the row as `role="alert"` instead of the "taken" state. */
  error?: string;
  /** A new shortcut was recorded (the canonical string `Ctrl+Alt+Shift+Cmd+<code>`). */
  onChange: (accelerator: string) => void;
  /** "Сбросить": return the default shortcut (or try to assign it again if it is taken). */
  onReset: () => void;
  /** Recording mode started or ended (Rust disables the global shortcuts themselves for that time, otherwise the key press would not reach the window). */
  onRecording: (active: boolean) => void;
}

/**
 * The "Сочетание" row inside a card-group: on the left the action name, on the right in one group the shortcut as key "capsules" (⌥ Space)
 * and the "Изменить" and "Сбросить" buttons (vertically centered; in a narrow window the group moves below the name), under the row the state: "Не назначено: занято…" or a save error.
 *
 * Recording a new shortcut: "Изменить" -> instead of the capsules the "Нажмите новое сочетание…" field, focus in it, `keydown` is listened to
 * with the browser action prevented. Only keys from the list are accepted (`acceleratorFromEvent`); everything else, including Tab,
 * Enter and arrows, is ignored and recording continues. Esc cancels, and so does losing focus. The settings window does not receive Esc
 * and Tab during recording (`stopPropagation`): Esc leaves recording rather than closing the window. The mode is announced to the screen reader (`role="status"`).
 */
export function ShortcutRow({ status, error, onChange, onReset, onRecording }: ShortcutRowProps) {
  const { t } = useTranslation();
  const nameId = useId();
  const [recording, setRecording] = useState(false);
  const recorder = useRef<HTMLDivElement>(null);
  const actions = useRef<HTMLDivElement>(null);
  const returnFocus = useRef(false);
  // A fresh handler without resubscribing: the parent may pass a new function on every render.
  const onRecordingRef = useRef(onRecording);
  useEffect(() => {
    onRecordingRef.current = onRecording;
  });

  // The start and end of recording (including unmounting and losing focus) are reported to the parent in one place.
  useEffect(() => {
    if (!recording) return;
    onRecordingRef.current(true);
    recorder.current?.focus();
    return () => onRecordingRef.current(false);
  }, [recording]);

  // After recording, focus returns to "Изменить" so that keyboard users do not lose their place (the buttons are hidden during recording).
  useEffect(() => {
    if (recording || !returnFocus.current) return;
    returnFocus.current = false;
    actions.current?.querySelector("button")?.focus();
  }, [recording]);

  // Recording ends once: losing focus because the recording field itself closes must not cancel returning focus to the button.
  const active = useRef(false);
  const start = () => {
    active.current = true;
    setRecording(true);
  };
  const stop = (refocus: boolean) => {
    if (!active.current) return;
    active.current = false;
    returnFocus.current = refocus;
    setRecording(false);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    // Everything pressed in recording mode belongs to the recording: neither Tab nor Esc reaches the settings window.
    event.preventDefault();
    event.stopPropagation();
    if (event.nativeEvent.isComposing) return;
    if (event.key === "Escape" && !event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey) {
      stop(true);
      return;
    }
    const accelerator = acceleratorFromEvent(event);
    if (accelerator === null) return;
    stop(true);
    onChange(accelerator);
  };

  const name = t(`settings.shortcuts.${status.kind}`);
  const taken = status.state === "taken";
  const { modifiers, key } = parseAccelerator(status.accelerator);
  const spoken = [...modifiers.map((modifier: Modifier) => t(`settings.shortcuts.modifier.${modifier.toLowerCase() as Lowercase<Modifier>}`)), ...(key ? [key] : [])].join(" + ");

  return (
    <div className="shortcut-row" role="group" aria-labelledby={nameId}>
      <div className="shortcut-row__line">
        <span className="shortcut-row__name" id={nameId}>
          {name}
        </span>
        <div className="shortcut-row__controls">
          {recording ? (
            <div
              className="shortcut-row__recorder"
              ref={recorder}
              tabIndex={0}
              role="group"
              aria-label={t("settings.shortcuts.recordingLabel", { name })}
              title={t("settings.shortcuts.recording")}
              onKeyDown={onKeyDown}
              onBlur={() => stop(false)}
            >
              <span aria-hidden="true">{t("settings.shortcuts.recordingShort")}</span>
            </div>
          ) : (
            <span
              className={`shortcut-row__keys${taken ? " shortcut-row__keys--taken" : ""}`}
              role="img"
              aria-label={t("settings.shortcuts.keys", { keys: spoken })}
            >
              {formatAccelerator(status.accelerator).map((cap, index) => (
                <kbd className="keycap" key={`${index}-${cap}`} aria-hidden="true">
                  {cap}
                </kbd>
              ))}
            </span>
          )}
          <div className={`shortcut-row__actions${recording ? " shortcut-row__actions--hidden" : ""}`} ref={actions}>
            <Button variant="secondary" aria-label={t("settings.shortcuts.changeLabel", { name })} onClick={start}>
              {t("settings.shortcuts.change")}
            </Button>
            {!status.isDefault || taken ? (
              <Button variant="accent" aria-label={t("settings.shortcuts.resetLabel", { name })} onClick={onReset}>
                {t("settings.shortcuts.reset")}
              </Button>
            ) : null}
          </div>
        </div>
      </div>
      {/* The recording mode announcement: the region is always there and the text appears when recording starts, otherwise the screen reader will not hear it.
          Without role="status": the settings window already has status lines, and this is not one of them. */}
      <span className="visually-hidden" aria-live="polite" aria-atomic="true">
        {recording ? t("settings.shortcuts.recording") : ""}
      </span>
      {error ? (
        <StatusNote tone="error" icon="alert" role="alert">
          {error}
        </StatusNote>
      ) : taken ? (
        <StatusNote tone="error" icon="alert" role="status">
          {t("settings.shortcuts.taken")}
        </StatusNote>
      ) : null}
    </div>
  );
}
