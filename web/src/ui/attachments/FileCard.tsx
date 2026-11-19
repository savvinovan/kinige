import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { useLanguage } from "../../i18n/I18n";
import { Icon } from "../icons/Icon";
import { fileIcon, fileMetaText } from "./fileMeta";
import { FileName } from "./fileName";
import "./file-card.css";

interface FileCardProps {
  /** A file, audio or video attachment; there are no players in this slice. */
  attachment: AttachmentView;
  /** A click or Enter: the file opens in its system app (Rust decides what is safe to open). Not called for a missing file. */
  onOpen: (attachmentId: string) => void;
  /** A right click on the card (the attachment menu). */
  onContextMenu?: (event: MouseEvent<HTMLElement>) => void;
}

/**
 * A file in the bubble (slice 7): an icon by kind, the name cut in the middle (the extension stays visible), the description in up to two lines
 * when there is one (KINIG-136), and "PDF · 2.4 MB" under it. The whole card is one button. A missing file keeps the same box: the name and
 * "File is missing", and the click does nothing.
 */
export function FileCard({ attachment, onOpen, onContextMenu }: FileCardProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const missing = attachment.state === "missing";
  const second = missing ? t("attachments.missing") : fileMetaText(attachment, language, t);
  const described = attachment.description ? `${attachment.name}. ${attachment.description}. ${second}` : `${attachment.name}. ${second}`;
  return (
    <button
      type="button"
      className={missing ? "file-card file-card--missing" : "file-card"}
      aria-label={described}
      title={attachment.name}
      aria-disabled={missing || undefined}
      onContextMenu={onContextMenu}
      onClick={() => {
        if (!missing) onOpen(attachment.id);
      }}
    >
      <span className="file-card__icon">
        <Icon name={fileIcon(attachment.kind)} />
      </span>
      <span className="file-card__text">
        <FileName name={attachment.name} className="file-card__name" />
        {attachment.description ? <span className="file-card__description">{attachment.description}</span> : null}
        <span className="file-card__meta">{second}</span>
      </span>
    </button>
  );
}
