import { useEffect, useRef, useState, type CSSProperties, type FormEvent, type MutableRefObject, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentDetailsChanges } from "../../api/generated/AttachmentDetailsChanges";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { Button } from "../controls/Button";
import { Checkbox } from "../controls/Checkbox";
import { IconButton } from "../controls/IconButton";
import { TextAreaField } from "../controls/TextAreaField";
import { TextField } from "../controls/TextField";
import { Icon } from "../icons/Icon";
import { Modal } from "../settings/Modal";
import { fileIcon } from "./fileMeta";
import { useMediaUrl } from "./mediaUrl";
import { PreviewTile } from "./PreviewTile";
import { useReveal } from "./reveal";
import { SpoilerFace } from "./SpoilerFace";
import "./details-dialog.css";

/** The counter appears from this many characters on; the limit itself is Rust's (1,024 scalar values after normalizing), the number here is advisory. */
const COUNTER_FROM = 900;
const COUNTER_MAX = 1024;

interface DetailsDialogProps {
  /** The attachments the dialog can step through (an entry's, or the draft's tiles), and the one on screen. */
  attachments: readonly AttachmentView[];
  index: number;
  onIndexChange: (index: number) => void;
  /** Sends the edited fields (only those) for one attachment; rejects with an error that `error` shows. */
  onSave: (attachmentId: string, changes: AttachmentDetailsChanges) => Promise<void>;
  onClose: () => void;
  /** Where the focus goes on close (the button the dialog was opened on); see `Modal`. */
  returnFocusRef?: RefObject<HTMLElement | null>;
  /** The translated error of the last save, shown under the fields. */
  error?: string;
}

/** The part of a display name before the fixed extension of the original (`beach.jpg` of `IMG_0001.jpg` is `beach`). */
export function stemOf(view: AttachmentView): string {
  const dot = view.filename.lastIndexOf(".");
  const suffix = dot > 0 ? view.filename.slice(dot) : "";
  return suffix !== "" && view.name.endsWith(suffix) ? view.name.slice(0, view.name.length - suffix.length) : view.name;
}

const suffixOf = (view: AttachmentView): string => {
  const dot = view.filename.lastIndexOf(".");
  return dot > 0 ? view.filename.slice(dot) : "";
};

/**
 * "Attachment details" (KINIG-136): the name (the stem, with the original extension as a fixed suffix), the description, and for a photo "Hide as spoiler".
 * The picture is on the left, covered when the attachment is hidden, with a "Show" button (the reveal state is shared with the feed). Enter in the name
 * saves, Escape cancels, an entry with several attachments shows "2 / 4" and the arrows. It sends only the fields that changed.
 */
export function DetailsDialog({ attachments, index, onIndexChange, onSave, onClose, returnFocusRef, error }: DetailsDialogProps) {
  const { t } = useTranslation();
  // A step by the arrow buttons remounts the form (a new attachment is a new form), and the button that was pressed goes with it: the new form
  // puts the focus on the same button again.
  const refocus = useRef<Step | null>(null);
  const current = attachments[Math.min(Math.max(index, 0), attachments.length - 1)];
  if (!current) return null;
  const position = attachments.indexOf(current);
  return (
    <Modal title={t("attachments.dialog.title")} closeLabel={t("attachments.dialog.close")} onClose={onClose} returnFocusRef={returnFocusRef}>
      <DetailsForm
        // A new attachment is a new form: its own fields, not the last one's.
        key={current.id}
        attachment={current}
        position={position}
        total={attachments.length}
        onStep={(direction, byButton) => {
          refocus.current = byButton ? direction : null;
          onIndexChange(position + direction);
        }}
        refocus={refocus}
        onSave={onSave}
        onClose={onClose}
        error={error}
      />
    </Modal>
  );
}

type Step = -1 | 1;

interface FormProps {
  attachment: AttachmentView;
  position: number;
  total: number;
  /** `byButton`: the step came from the arrow button, not the keyboard arrows. */
  onStep: (direction: Step, byButton: boolean) => void;
  refocus: MutableRefObject<Step | null>;
  onSave: DetailsDialogProps["onSave"];
  onClose: () => void;
  error?: string;
}

function DetailsForm({ attachment, position, total, onStep, refocus, onSave, onClose, error }: FormProps) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const { isRevealed, reveal } = useReveal();
  const initial = { stem: stemOf(attachment), description: attachment.description ?? "", spoiler: attachment.spoiler };
  const [stem, setStem] = useState(initial.stem);
  const [description, setDescription] = useState(initial.description);
  const [spoiler, setSpoiler] = useState(initial.spoiler);
  const [busy, setBusy] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const image = attachment.kind === "image";
  const covered = image && attachment.spoiler && !isRevealed(attachment.id);

  /** Only what was edited; `null` if nothing was. */
  const changes = (): AttachmentDetailsChanges | null => {
    const out: AttachmentDetailsChanges = {};
    if (stem !== initial.stem) out.name = stem;
    if (description !== initial.description) out.description = description;
    if (image && spoiler !== initial.spoiler) out.spoiler = spoiler;
    return Object.keys(out).length === 0 ? null : out;
  };
  const save = async (then: () => void) => {
    const edited = changes();
    if (edited === null) return then();
    setBusy(true);
    try {
      await onSave(attachment.id, edited);
      then();
    } catch {
      // The parent shows the error; the fields stay as they were typed.
    } finally {
      setBusy(false);
    }
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!busy) void save(onClose);
  };
  const panel = () => form.current?.closest<HTMLElement>(".modal__panel") ?? null;
  // The arrows move between attachments, except in a text field, where they move the caret. They are heard on the whole window, not on the form:
  // the window puts the focus on the header's close button first, which is not inside the form.
  const keys = (event: KeyboardEvent) => {
    // A text field keeps the arrows for its caret; a checkbox does not use them.
    const inField =
      event.target instanceof HTMLTextAreaElement || (event.target instanceof HTMLInputElement && !["checkbox", "radio", "button"].includes(event.target.type));
    const inside = event.target instanceof Node && panel()?.contains(event.target) === true;
    if (!inside || inField || event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const direction = event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : 0;
    if (direction !== 0 && position + direction >= 0 && position + direction < total) {
      event.preventDefault();
      void save(() => onStep(direction, false));
    }
  };
  const latestKeys = useRef(keys);
  useEffect(() => {
    latestKeys.current = keys;
  });
  useEffect(() => {
    const listener = (event: KeyboardEvent) => latestKeys.current(event);
    document.addEventListener("keydown", listener);
    return () => document.removeEventListener("keydown", listener);
  }, []);
  // A form that replaced the last one (a step) takes the focus the removed button had: the same arrow button, or the window itself.
  useEffect(() => {
    const want = refocus.current;
    refocus.current = null;
    const button = want === null ? null : form.current?.querySelector<HTMLElement>(`[data-step="${want}"]`);
    if (button) button.focus();
    else if (document.activeElement === document.body) panel()?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const count = [...description].length;
  const shape = attachment.width && attachment.height ? ({ "--details-aspect": `${attachment.width / attachment.height}` } as CSSProperties) : undefined;

  return (
    <form className="details" ref={form} onSubmit={submit}>
      <div className="details__picture">
        <div className="details__frame" style={shape}>
          {image ? (
            covered ? (
              <SpoilerFace />
            ) : attachment.state === "missing" ? (
              <PreviewTile reason="missing" />
            ) : (
              <img className="details__image" src={mediaUrl("thumb", attachment.id)} alt="" draggable={false} />
            )
          ) : (
            <span className="details__file">
              <Icon name={fileIcon(attachment.kind)} />
            </span>
          )}
        </div>
        {covered ? (
          <Button
            variant="secondary"
            aria-label={t("attachments.showLabel")}
            onClick={() => {
              // The button goes with the cover: the focus goes to the window first (as in the viewer), or it would fall to the page.
              panel()?.focus();
              reveal(attachment.id);
            }}
          >
            {t("attachments.show")}
          </Button>
        ) : null}
        {total > 1 ? (
          <div className="details__steps">
            <IconButton
              icon="chevronLeft"
              label={t("attachments.dialog.previous")}
              aria-disabled={position === 0 || undefined}
              data-step="-1"
              onClick={() => position > 0 && void save(() => onStep(-1, true))}
            />
            <span className="details__position" aria-live="polite">
              {t("attachments.dialog.position", { index: position + 1, total })}
            </span>
            <IconButton
              icon="chevronRight"
              label={t("attachments.dialog.next")}
              aria-disabled={position === total - 1 || undefined}
              data-step="1"
              onClick={() => position < total - 1 && void save(() => onStep(1, true))}
            />
          </div>
        ) : null}
      </div>
      <div className="details__fields">
        <TextField caption={t("attachments.dialog.name")} label={t("attachments.dialog.name")} value={stem} suffix={suffixOf(attachment)} disabled={busy} onChange={(event) => setStem(event.target.value)} />
        <TextAreaField
          caption={t("attachments.dialog.description")}
          label={t("attachments.dialog.description")}
          value={description}
          disabled={busy}
          note={count >= COUNTER_FROM ? t("attachments.dialog.descriptionCount", { count, max: COUNTER_MAX }) : undefined}
          onChange={(event) => setDescription(event.target.value)}
        />
        {image ? (
          <Checkbox checked={spoiler} disabled={busy} description={t("attachments.dialog.spoilerNote")} onChange={(event) => setSpoiler(event.target.checked)}>
            {t("attachments.dialog.spoiler")}
          </Checkbox>
        ) : null}
        {error ? (
          <p className="details__error" role="alert">
            {error}
          </p>
        ) : null}
        <div className="details__buttons">
          <Button variant="secondary" onClick={onClose}>
            {t("attachments.dialog.cancel")}
          </Button>
          <Button type="submit" disabled={busy}>
            {t("attachments.dialog.save")}
          </Button>
        </div>
      </div>
    </form>
  );
}
