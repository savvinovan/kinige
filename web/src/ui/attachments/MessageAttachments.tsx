import { useState, type MouseEvent } from "react";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import type { MenuPosition } from "../menu/ContextMenu";
import { VoicePlayer } from "../voice/VoicePlayer";
import { Album } from "./Album";
import { AttachmentMenu } from "./AttachmentMenu";
import { FileCard } from "./FileCard";
import { useReveal } from "./reveal";
import "./message-attachments.css";

interface MessageAttachmentsProps {
  attachments: readonly AttachmentView[];
  /** A photo was opened: its attachment id (the feed knows the entry's other photos and opens the lightbox on it). */
  onOpenImage: (attachmentId: string) => void;
  /** A file card was opened: the file opens in its system app. */
  onOpenFile: (attachmentId: string) => void;
  /** "Details…" of an attachment (the context menu): the window opens the dialog on it. Without it the menu has no such item. */
  onDetails?: (attachmentId: string) => void;
  /** "Hide as spoiler" / "Show without spoiler" of a photo: only the spoiler is sent. */
  onSpoiler?: (attachmentId: string, spoiler: boolean) => void;
  /** Extra class on the root: the bubble uses it to size itself when the entry has no text. */
  className?: string;
}

/**
 * The attachments of an entry inside its bubble, above the text (slice 7): the photos as one album in order, then every other attachment
 * as a file card, in order; an `audio` attachment (a voice note, slice 7a) is a player instead. It has no background or border (the bubble gives them); the root class `message-attachments`
 * lets the bubble find it, e.g. to size itself when the entry has only attachments. Nothing for an entry without attachments.
 *
 * A right click on a photo or a file card opens its own menu (KINIG-136): "Details…" and, for a photo, the spoiler. A voice note has no details.
 */
export function MessageAttachments({ attachments, onOpenImage, onOpenFile, onDetails, onSpoiler, className }: MessageAttachmentsProps) {
  const { hide } = useReveal();
  const [menu, setMenu] = useState<{ attachment: AttachmentView; position: MenuPosition } | null>(null);
  const images = attachments.filter((attachment) => attachment.kind === "image");
  const files = attachments.filter((attachment) => attachment.kind !== "image");
  if (images.length === 0 && files.length === 0) return null;

  const hasMenu = onDetails !== undefined || onSpoiler !== undefined;
  const openMenu = (attachment: AttachmentView | undefined, event: MouseEvent<HTMLElement>) => {
    if (!attachment || !hasMenu || attachment.kind === "audio") return;
    // The entry's own menu is for the rest of the bubble: this one is the attachment's.
    event.preventDefault();
    event.stopPropagation();
    setMenu({ attachment, position: { x: event.clientX, y: event.clientY } });
  };
  return (
    <div className={className ? `message-attachments ${className}` : "message-attachments"}>
      {images.length > 0 ? (
        <Album
          images={images}
          onOpen={(index) => {
            const image = images[index];
            if (image) onOpenImage(image.id);
          }}
          onContextMenu={hasMenu ? (index, event) => openMenu(images[index], event) : undefined}
        />
      ) : null}
      {files.map((file) =>
        file.kind === "audio" ? (
          <VoicePlayer key={file.id} attachment={file} onOpenFile={onOpenFile} />
        ) : (
          <FileCard key={file.id} attachment={file} onOpen={onOpenFile} onContextMenu={hasMenu ? (event) => openMenu(file, event) : undefined} />
        ),
      )}
      {menu ? (
        <AttachmentMenu
          attachment={menu.attachment}
          position={menu.position}
          onClose={() => setMenu(null)}
          onDetails={() => onDetails?.(menu.attachment.id)}
          onSpoiler={(spoiler) => {
            // A photo that was shown before is covered at once, not only flagged: the reveal is forgotten with the flag.
            if (spoiler) hide(menu.attachment.id);
            onSpoiler?.(menu.attachment.id, spoiler);
          }}
        />
      ) : null}
    </div>
  );
}
