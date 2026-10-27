import { useEffect, useId, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import type { Language } from "../../i18n";
import { Button } from "../controls/Button";
import { TextField } from "../controls/TextField";
import { EmojiPicker } from "../emoji/EmojiPicker";
import { Modal } from "../settings/Modal";
import "./chat-dialog.css";

/** Название чата — от 1 до 128 знаков (ТЗ слайса 3); считаются символы, а не кодовые единицы UTF-16. */
export const CHAT_NAME_MAX = 128;

/** Эмодзи по умолчанию: подсказка на кнопке, пока человек ничего не выбрал; Rust подставляет его же. */
const DEFAULT_EMOJI = "💬";

export interface ChatDialogProps {
  /** create — «Новый чат»; rename — «Переименовать чат»; icon — только пикер: выбор эмодзи сразу отправляет. */
  mode: "create" | "rename" | "icon";
  /** Имя в поле (rename) и имя, которое уходит вместе с эмодзи (icon). */
  initialName?: string;
  /** Текущий эмодзи чата. Окно его не меняет, пока человек не выбрал другой: `onSubmit` получит `null`. */
  initialEmoji?: string | null;
  recentEmoji: readonly string[];
  language: Language;
  /** Уже переведённая ошибка Rust (пустое имя и подобное); показывается строкой `role="alert"`, ввод остаётся. */
  error?: string;
  /** Запрос идёт: поля и кнопки недоступны. */
  busy?: boolean;
  /** `emoji` — `null`, если человек эмодзи не выбирал (create: Rust подставит 💬; rename: эмодзи не трогается). */
  onSubmit: (name: string, emoji: string | null) => void;
  onClose: () => void;
}

const clip = (text: string) => {
  const symbols = Array.from(text);
  return symbols.length > CHAT_NAME_MAX ? symbols.slice(0, CHAT_NAME_MAX).join("") : text;
};

/**
 * Окно чата поверх главного (Modal): создание, переименование и смена эмодзи. Поле «Название» с автофокусом, Enter
 * отправляет, пустое имя — кнопка недоступна. Эмодзи выбирается в пикере, который раскрывается под полем: окно прижато к
 * верху, поэтому заголовок не сдвигается. Esc закрывает пикер, а следующий закрывает окно. Пикер стоит вне `<form>`: Enter в его
 * поиске не должен отправлять форму.
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

  // Modal при открытии ставит фокус на кнопку закрытия; эффект родителя выполняется позже, поэтому фокус переходит в поле
  // (или в поиск пикера в режиме «эмодзи»).
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

  // Esc внутри пикера (создание): закрывает только пикер; окно закроется следующим Esc.
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
