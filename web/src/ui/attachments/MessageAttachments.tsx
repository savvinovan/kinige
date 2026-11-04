import type { AttachmentView } from "../../api/generated/AttachmentView";
import { Album } from "./Album";
import { FileCard } from "./FileCard";
import "./message-attachments.css";

interface MessageAttachmentsProps {
  attachments: readonly AttachmentView[];
  /** A photo was opened: its attachment id (the feed knows the entry's other photos and opens the lightbox on it). */
  onOpenImage: (attachmentId: string) => void;
  /** A file card was opened: the file opens in its system app. */
  onOpenFile: (attachmentId: string) => void;
  /** Extra class on the root: the bubble uses it to size itself when the entry has no text. */
  className?: string;
}

/**
 * The attachments of an entry inside its bubble, above the text (slice 7): the photos as one album in order, then every other attachment
 * as a file card, in order. It has no background or border (the bubble gives them); the root class `message-attachments`
 * lets the bubble find it, e.g. to size itself when the entry has only attachments. Nothing for an entry without attachments.
 */
export function MessageAttachments({ attachments, onOpenImage, onOpenFile, className }: MessageAttachmentsProps) {
  const images = attachments.filter((attachment) => attachment.kind === "image");
  const files = attachments.filter((attachment) => attachment.kind !== "image");
  if (images.length === 0 && files.length === 0) return null;
  return (
    <div className={className ? `message-attachments ${className}` : "message-attachments"}>
      {images.length > 0 ? (
        <Album
          images={images}
          onOpen={(index) => {
            const image = images[index];
            if (image) onOpenImage(image.id);
          }}
        />
      ) : null}
      {files.map((file) => (
        <FileCard key={file.id} attachment={file} onOpen={onOpenFile} />
      ))}
    </div>
  );
}
