import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type KinigeApi, errorCode } from "../api/KinigeApi";
import type { CaptureState } from "../api/generated/CaptureState";
import type { ErrorCode } from "../api/generated/ErrorCode";
import { useDraftSaver } from "../draft/useDraftSaver";
import { applyTextSize } from "../ui/screens/textSize";
import { CapturePanel } from "../ui/capture/CapturePanel";
import { useInputModality } from "../ui/useInputModality";

/** Rust showed the window (see `capture::show_capture`): the page rereads the state. Not an IPC event: the window has no right to listen. */
export const SHOWN_EVENT = "kinige-capture-shown";
/** Rust asks to hide the window (the hotkey with the window open, closing, loss of focus): the page saves the draft and hides. */
export const HIDE_REQUEST_EVENT = "kinige-capture-hide-request";

interface CaptureWindowProps {
  api: KinigeApi;
  /** The state read before the first frame. */
  initial: CaptureState;
}

/**
 * The quick capture window (slice 2) on `KinigeApi`: today's context, the input field, Enter - a new entry, Cmd+Enter -
 * append, Esc - hide. The draft goes to Rust ~300 ms after the last keypress and on hiding. After success - silence: the
 * window just hides (no toast, no confirmation). A write error leaves the text in the field, with a message above it.
 */
export function CaptureWindow({ api, initial }: CaptureWindowProps) {
  const { t } = useTranslation();
  useInputModality();
  const [state, setState] = useState(initial);
  const [value, setValue] = useState(initial.draft);
  const [error, setError] = useState<ErrorCode | null>(initial.error);
  const busy = useRef(false);
  // The emoji picker is open: the panel grows upward (the window is fitted by the `capture_resize` command). It closes on every window hide.
  const [pickerOpen, setPickerOpen] = useState(false);

  const saveDraft = useCallback((body: string) => api.captureSaveDraft(body), [api]);
  const draft = useDraftSaver(saveDraft);
  const draftSaver = draft;
  const { flush, cancel, schedule } = draft;
  // The show counter: after every show (and the first one) the cursor is put at the end of the text, as in the main window.
  const [shown, setShown] = useState(0);
  useEffect(() => {
    const field = document.querySelector("textarea");
    if (!field) return;
    field.focus();
    field.setSelectionRange(field.value.length, field.value.length);
  }, [shown]);

  /** The window height follows the content; a resize failure does not prevent writing (the window keeps its old size). */
  const resize = useCallback((height: number) => void api.captureResize(height).catch(() => undefined), [api]);

  /** Saves the draft and hides the window. */
  const hide = useCallback(() => {
    setPickerOpen(false);
    void flush().finally(() => void api.captureHide().catch(() => undefined));
  }, [api, flush]);

  // On show: a fresh context and draft (entries could appear from the main window, the clipboard and another screen), the text size.
  useEffect(() => {
    const refresh = () => {
      api
        .captureState()
        .then((fresh) => {
          applyTextSize(fresh.textSize);
          setState(fresh);
          setPickerOpen(false);
          setValue(fresh.draft);
          setError(fresh.error);
          // The window was shown again: the page could consider the focus its own all this time (there is no focus
          // event), so the cursor is put into the field explicitly - after the value is already in the field (an effect on the counter).
          setShown((n) => n + 1);
        })
        .catch((reason: unknown) => setError(errorCode(reason)));
    };
    window.addEventListener(SHOWN_EVENT, refresh);
    window.addEventListener(HIDE_REQUEST_EVENT, hide);
    return () => {
      window.removeEventListener(SHOWN_EVENT, refresh);
      window.removeEventListener(HIDE_REQUEST_EVENT, hide);
    };
  }, [api, hide]);

  const submit = (action: (body: string) => ReturnType<KinigeApi["captureSend"]>) => {
    if (busy.current || value.trim() === "") return;
    busy.current = true;
    const text = value;
    // While sending, input is not accepted (see `onChange`) and the draft is not saved: Rust clears it itself, in the same
    // transaction. First the scheduled save is cancelled and the already sent save is awaited - otherwise it would reach
    // the writer after the send and bring the sent text back as a draft.
    cancel();
    draftSaver
      .settle()
      .then(() => action(text))
      .then(() => {
        cancel();
        setValue("");
        setError(null);
        setPickerOpen(false);
        void api.captureHide().catch(() => undefined);
      })
      .catch((reason: unknown) => {
        // The text stays in the field, with an error above it; the draft goes back to the save queue.
        setError(errorCode(reason));
        schedule(text);
      })
      .finally(() => {
        busy.current = false;
      });
  };

  return (
    <CapturePanel
      context={state.context}
      appendTargetId={state.appendTarget}
      value={value}
      onChange={(next) => {
        // While the entry goes to Rust, keypresses are not accepted: what is typed at that moment would otherwise come back as a draft.
        if (busy.current) return;
        setValue(next);
        setError(null);
        schedule(next);
      }}
      onSend={() => submit((body) => api.captureSend(body))}
      onAppend={() => submit((body) => api.captureAppend(body))}
      onHide={hide}
      onHeight={resize}
      error={error ? t(`error.${error}`) : undefined}
      emoji={{
        recent: state.recentEmoji,
        // The choice is inserted into the field; it reaches "recent" (shared with the main window) by a separate command, whose failure does not prevent writing.
        onPicked: (emoji) =>
          void api
            .captureAddRecentEmoji(emoji)
            .then((recentEmoji) => setState((old) => ({ ...old, recentEmoji })))
            .catch(() => undefined),
        open: pickerOpen,
        onOpenChange: setPickerOpen,
      }}
    />
  );
}
