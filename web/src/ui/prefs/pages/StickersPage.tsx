import { useId, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import type { ErrorCode } from "../../../api/generated/ErrorCode";
import { Button } from "../../controls/Button";
import { TextField } from "../../controls/TextField";
import { StatusNote } from "../../settings/StatusNote";
import "../../settings/settings.css";

/** What the Stickers page shows of the Telegram bot token and what it can do: the token itself is never here after it is sent. */
export interface TokenControls {
  /** Whether a token is saved (the flag only); `null` until it is read. */
  saved: boolean | null;
  /** A save or a removal is under way. */
  busy: boolean;
  /** The error of the last save or removal. */
  error: ErrorCode | null;
  onSave: (token: string) => void;
  onRemove: () => void;
}

/** The page with no window behind it (stories of the whole settings window): an empty field that does nothing. */
export const NO_TOKEN_CONTROLS: TokenControls = { saved: false, busy: false, error: null, onSave: () => undefined, onRemove: () => undefined };

/**
 * "Stickers": the line about what the bot is for, a password-style field "Telegram bot token" with Save, and once a token is saved "Token saved" with
 * Remove (slice 5c). Two quiet notes: what Telegram learns, and the keychain. The field is cleared as soon as Save is pressed, whatever the answer, so the
 * token does not stay on screen or in the window's state; an error stands under it as one line. Until a token is saved Kinige makes no request to Telegram.
 */
export function StickersPage({ saved, busy, error, onSave, onRemove }: TokenControls) {
  const { t } = useTranslation();
  const titleId = useId();
  const [token, setToken] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const value = token.trim();
    if (value === "" || busy) return;
    setToken("");
    onSave(value);
  };
  return (
    <section className="prefs-page" aria-labelledby={titleId}>
      <h2 className="prefs-page__title" id={titleId}>
        {t("prefs.section.stickers")}
      </h2>
      <div className="prefs-block">
        <h3 className="settings__heading">{t("settings.stickers.title")}</h3>
        <p className="settings__description">{t("settings.stickers.description")}</p>
        {saved === true ? (
          <div className="token-row">
            <StatusNote tone="quiet" icon="done" role="status">
              {t("settings.stickers.saved")}
            </StatusNote>
            <Button variant="secondary" aria-label={t("settings.stickers.removeLabel")} aria-disabled={busy || undefined} onClick={() => !busy && onRemove()}>
              {t("settings.stickers.remove")}
            </Button>
          </div>
        ) : null}
        {saved === false ? (
          <form className="token-row" onSubmit={submit}>
            <TextField
              className="token-row__field"
              type="password"
              label={t("settings.stickers.tokenLabel")}
              placeholder={t("settings.stickers.tokenLabel")}
              value={token}
              onChange={(event) => setToken(event.target.value)}
              autoComplete="off"
              autoCapitalize="off"
              spellCheck={false}
            />
            <Button variant="secondary" type="submit" aria-disabled={busy || token.trim() === "" || undefined}>
              {busy ? t("settings.stickers.saving") : t("settings.stickers.save")}
            </Button>
          </form>
        ) : null}
        {error ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {t(`error.${error}`)}
          </StatusNote>
        ) : null}
        <StatusNote tone="quiet" icon="info">
          {t("settings.stickers.learns")}
        </StatusNote>
        <StatusNote tone="quiet" icon="info">
          {t("settings.stickers.keychain")}
        </StatusNote>
      </div>
    </section>
  );
}
