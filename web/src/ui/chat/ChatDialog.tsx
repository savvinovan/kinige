import { useEffect, useId, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { Button } from "../controls/Button";
import { TextField } from "../controls/TextField";
import { EmojiPicker } from "../emoji/EmojiPicker";
import { Modal } from "../settings/Modal";
import "./chat-dialog.css";

/** A chat name is 1 to 128 characters (slice 3 spec); characters are counted, not UTF-16 code units. */
export const CHAT_NAME_MAX = 128;

/** The default emoji: a hint on the button while the person has chosen nothing; Rust substitutes the same one. */
const DEFAULT_EMOJI = "💬";

export interface ChatDialogProps {
  /** create is "New chat"; rename is "Rename chat"; icon is the picker only: choosing an emoji submits at once. */
  mode: "create" | "rename" | "icon";
  /** The name in the field (rename) and the name that goes together with the emoji (icon). */
  initialName?: string;
  /** The chat's current emoji. The window does not change it until the person chooses another: `onSubmit` gets `null`. */
  initialEmoji?: string | null;
  recentEmoji: readonly string[];
  language: Language;
  /** An already translated Rust error (an empty name and the like); shown as a `role="alert"` line, the input stays. */
  error?: string;
  /** A request is in flight: the fields and buttons are disabled. */
  busy?: boolean;
  /** `emoji` is `null` if the person did not choose an emoji (create: Rust substitutes 💬; rename: the emoji is not touched). */
  onSubmit: (name: string, emoji: string | null) => void;
  onClose: () => void;
}

const clip = (text: string) => {
  const symbols = Array.from(text);
  return symbols.length > CHAT_NAME_MAX ? symbols.slice(0, CHAT_NAME_MAX).join("") : text;
};

/**
 * The chat window over the main one (Modal): create, rename and change the emoji. The "Name" field with autofocus, Enter
 * submits, an empty name disables the button. The emoji is chosen in the picker, which opens under the field: the window is pinned to
 * the top, so the heading does not shift. Esc closes the picker, and the next one closes the window. The picker stands outside the `<form>`: Enter in its
 * search must not submit the form.
 */
export function ChatDialog({
  mode,
  initialName = "",
  initialEmoji = null,
  recentEmoji,
  language,
  error,
  busy = false,
  onSubmit,
  onClose,
}: ChatDialogProps) {
  const { t } = useTranslation();
  const root = useRef<HTMLDivElement>(null);
  const emojiButton = useRef<HTMLButtonElement>(null);
  const pickerId = useId();
  const formId = useId();
  const [name, setName] = useState(mode === "create" ? "" : initialName);
  const [emoji, setEmoji] = useState<string | null>(null);
  const [pickerOpen, setPickerOpen] = useState(mode === "icon");
  const trimmed = name.trim();
  const canSubmit = trimmed !== "" && !busy;

  // On opening, Modal puts focus on the close button; the parent's effect runs later, so focus moves to the field
  // (or to the picker search in "emoji" mode).
  useEffect(() => {
    root.current?.querySelector("input")?.focus();
  }, []);

  const title = mode === "create" ? t("chatDialog.titleCreate") : mode === "rename" ? t("chatDialog.titleRename") : t("chatDialog.titleIcon");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (canSubmit) onSubmit(trimmed, mode === "create" ? emoji : null);
  };

  const pick = (picked: string) => {
    if (busy) return;
    if (mode === "icon") {
      onSubmit(initialName, picked);
      return;
    }
    setEmoji(picked);
    setPickerOpen(false);
    root.current?.querySelector("input")?.focus();
  };

  // Esc inside the picker (create): closes only the picker; the window closes on the next Esc.
  const onPickerKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Escape" || mode === "icon") return;
    event.preventDefault();
    event.stopPropagation();
    setPickerOpen(false);
    emojiButton.current?.focus();
  };

  return (
    <Modal title={title} closeLabel={t("common.close")} onClose={onClose}>
      <div className="chat-dialog" ref={root} aria-busy={busy || undefined}>
        {mode === "icon" ? (
          error ? (
            <p className="chat-dialog__error" role="alert">
              {error}
            </p>
          ) : null
        ) : (
          <form className="chat-dialog__field" id={formId} onSubmit={submit}>
            {mode === "create" ? (
              <button
                type="button"
                className={`chat-dialog__emoji${emoji ? "" : " chat-dialog__emoji--empty"}`}
                ref={emojiButton}
                aria-label={emoji ? t("chatDialog.emoji", { emoji }) : t("chatDialog.emojiChoose")}
                aria-expanded={pickerOpen}
                aria-controls={pickerOpen ? pickerId : undefined}
                disabled={busy}
                onClick={() => setPickerOpen((was) => !was)}
              >
                {emoji ?? initialEmoji ?? DEFAULT_EMOJI}
              </button>
            ) : null}
            <TextField
              className="chat-dialog__name"
              label={t("chatDialog.name")}
              placeholder={t("chatDialog.name")}
              value={name}
              error={error}
              disabled={busy}
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setName(clip(event.target.value))}
            />
          </form>
        )}
        {pickerOpen ? (
          <div className="chat-dialog__picker" id={pickerId} onKeyDown={onPickerKeyDown}>
            <EmojiPicker recent={recentEmoji} language={language} autoFocus onPick={pick} />
          </div>
        ) : null}
        {mode === "icon" ? null : (
          <div className="chat-dialog__actions">
            <Button variant="secondary" onClick={onClose}>
              {t("chatDialog.cancel")}
            </Button>
            <Button variant="accent" className="chat-dialog__submit" type="submit" form={formId} disabled={!canSubmit}>
              {mode === "create" ? t("chatDialog.create") : t("chatDialog.save")}
            </Button>
          </div>
        )}
      </div>
    </Modal>
  );
}
