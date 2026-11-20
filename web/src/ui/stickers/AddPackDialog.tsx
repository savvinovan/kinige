import { useEffect, useRef, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { StickerPackAdded } from "../../api/generated/StickerPackAdded";
import { Button } from "../controls/Button";
import { ProgressBar } from "../controls/ProgressBar";
import { TextField } from "../controls/TextField";
import { Modal } from "../settings/Modal";
import { StatusNote } from "../settings/StatusNote";
import "./add-pack.css";

/** What the dialog shows: the form (with the error of the last try), the running job (the counts are `null` until Telegram's first answer), or the result. */
export type AddPackState =
  | { phase: "idle"; error: ErrorCode | null }
  | { phase: "running"; done: number | null; total: number | null; cancelling: boolean }
  | { phase: "done"; result: StickerPackAdded };

export interface AddPackDialogProps {
  state: AddPackState;
  /** Whether a bot token is saved: without it the dialog says where to put one and starts nothing. */
  hasToken: boolean;
  link: string;
  onLinkChange: (link: string) => void;
  /** "Add": the job starts (the window checks nothing but that there is a link; Rust decides the rest). */
  onAdd: () => void;
  /** "Cancel" while the job runs: it is stopped and nothing is added. */
  onCancelJob: () => void;
  /** The cross, Esc, the backdrop, "Cancel" on the form and "Done". Not offered while the job runs. */
  onClose: () => void;
  /** "Open Settings" on the no-token line. */
  onOpenSettings: () => void;
}

/**
 * The "Add a sticker pack" dialog (slice 5c, the shared `Modal`): a field for the link, Add and Cancel, then the progress line "Downloading 12 of 30"
 * with Cancel, then one line, "Added «Test Pack»: 27 stickers. 3 animated or video stickers were skipped.", or the error as one line. Without a bot token
 * it says "Add your bot token in Settings → Stickers" with "Open Settings" and starts nothing. Esc closes it when no job runs. Nothing is added on
 * Cancel or on an error.
 */
export function AddPackDialog({ state, hasToken, link, onLinkChange, onAdd, onCancelJob, onClose, onOpenSettings }: AddPackDialogProps) {
  const { t } = useTranslation();
  const running = state.phase === "running";
  const noToken = !hasToken || (state.phase === "idle" && state.error === "sticker_pack_no_token");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (state.phase === "idle" && !noToken && link.trim() !== "") onAdd();
  };
  return (
    <Modal title={t("stickers.add.title")} closeLabel={t("common.close")} onClose={onClose} closable={!running}>
      <div className="add-pack">
        {state.phase === "idle" && noToken ? <NoToken onOpenSettings={onOpenSettings} onClose={onClose} /> : null}
        {state.phase === "idle" && !noToken ? (
          <Form state={state} link={link} onLinkChange={onLinkChange} onSubmit={submit} onClose={onClose} onOpenSettings={onOpenSettings} />
        ) : null}
        {state.phase === "running" ? <Running state={state} onCancel={onCancelJob} /> : null}
        {state.phase === "done" ? <Done result={state.result} onClose={onClose} /> : null}
      </div>
    </Modal>
  );
}

function NoToken({ onOpenSettings, onClose }: { onOpenSettings: () => void; onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <>
      <StatusNote tone="quiet" icon="info" role="status">
        {t("error.sticker_pack_no_token")}
      </StatusNote>
      <div className="add-pack__actions">
        <Button variant="secondary" onClick={onOpenSettings}>
          {t("stickers.add.openSettings")}
        </Button>
        <Button variant="accent" onClick={onClose}>
          {t("stickers.add.cancel")}
        </Button>
      </div>
    </>
  );
}

interface FormProps {
  state: Extract<AddPackState, { phase: "idle" }>;
  link: string;
  onLinkChange: (link: string) => void;
  onSubmit: (event: FormEvent) => void;
  onClose: () => void;
  onOpenSettings: () => void;
}

function Form({ state, link, onLinkChange, onSubmit, onClose, onOpenSettings }: FormProps) {
  const { t } = useTranslation();
  return (
    <form className="add-pack__form" onSubmit={onSubmit}>
      <TextField
        label={t("stickers.add.link")}
        placeholder={t("stickers.add.placeholder")}
        value={link}
        onChange={(event) => onLinkChange(event.target.value)}
        autoComplete="off"
        autoCapitalize="off"
        spellCheck={false}
      />
      {state.error ? (
        <StatusNote tone="error" icon="alert" role="alert">
          {t(`error.${state.error}`)}
        </StatusNote>
      ) : null}
      <div className="add-pack__actions">
        {state.error === "secret_store_denied" ? (
          <Button variant="secondary" onClick={onOpenSettings}>
            {t("stickers.add.openSettings")}
          </Button>
        ) : null}
        <Button variant="accent" onClick={onClose}>
          {t("stickers.add.cancel")}
        </Button>
        <Button variant="secondary" type="submit" aria-disabled={link.trim() === "" || undefined}>
          {t("stickers.add.add")}
        </Button>
      </div>
    </form>
  );
}

function Running({ state, onCancel }: { state: Extract<AddPackState, { phase: "running" }>; onCancel: () => void }) {
  const { t } = useTranslation();
  const cancel = useRef<HTMLButtonElement>(null);
  // The Add button that had focus is gone, and the dialog has no cross while a job runs: focus moves to Cancel.
  useEffect(() => cancel.current?.focus(), []);
  const known = state.done !== null && state.total !== null;
  const text = known ? t("stickers.add.progress", { done: state.done, total: state.total }) : t("stickers.add.starting");
  return (
    <>
      <p className="add-pack__text" role="status">
        {text}
      </p>
      <ProgressBar value={state.done ?? 0} max={state.total ?? 0} label={t("stickers.add.progressLabel")} valueText={text} />
      <div className="add-pack__actions">
        <Button variant="secondary" ref={cancel} aria-disabled={state.cancelling || undefined} onClick={() => !state.cancelling && onCancel()}>
          {t("stickers.add.cancel")}
        </Button>
      </div>
    </>
  );
}

function Done({ result, onClose }: { result: StickerPackAdded; onClose: () => void }) {
  const { t } = useTranslation();
  const done = useRef<HTMLButtonElement>(null);
  useEffect(() => done.current?.focus(), []);
  const added = t("stickers.add.added", { title: result.title, count: result.added });
  const skipped = result.skipped > 0 ? ` ${t("stickers.add.skipped", { count: result.skipped })}` : "";
  return (
    <>
      <p className="add-pack__text" role="status">
        {`${added}${skipped}`}
      </p>
      <div className="add-pack__actions">
        <Button variant="secondary" ref={done} onClick={onClose}>
          {t("stickers.add.done")}
        </Button>
      </div>
    </>
  );
}
