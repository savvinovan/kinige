import { useCallback, useEffect, useRef, useState } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { AttachOutcome } from "../../api/generated/AttachOutcome";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { AttachmentControls } from "./MainWindowView";

interface UseAttachmentsOptions {
  api: KinigeApi;
  /** The open chat; `null` means the trash is open, nothing can be attached. */
  selected: string | null;
  /** The unsent attachments of the open chat and how to change them (`useComposerDraft`). */
  tiles: readonly AttachmentView[];
  addTiles: (added: readonly AttachmentView[]) => void;
  dropTile: (attachmentId: string) => void;
  /** The error above the composer; `null` clears it. */
  onError: (code: ErrorCode | null) => void;
}

/** A photo being viewed: all the photos of its entry and which one is on screen. */
export interface LightboxState {
  images: AttachmentView[];
  index: number;
}

/** A generic name a browser gives a pasted image ("image.png"): the hint is dropped so that Rust names it `pasted-HHMMSS.<ext>`. */
const GENERIC_PASTE_NAME = /^image\.(png|jpe?g|gif|webp|tiff?)$/i;

/**
 * Attachments in the main window (slice 7): the paperclip dialog, a paste, a drop from Finder (Rust gives a one-time token, never
 * a path), removing a tile, the "Drop to attach" overlay, opening a photo in the lightbox and a file in its system app. The
 * files themselves are handled in Rust; here is only the state of the screen.
 */
export function useAttachments({ api, selected, tiles, addTiles, dropTile, onError }: UseAttachmentsOptions) {
  const [dragOver, setDragOver] = useState(false);
  const [lightbox, setLightbox] = useState<LightboxState | null>(null);
  const latest = useRef({ selected });
  useEffect(() => {
    latest.current = { selected };
  });

  const apply = useCallback(
    (outcome: AttachOutcome) => {
      addTiles(outcome.attached);
      // The tiles that made it stay; the first error (or "Folders are not attached") stands above the field.
      onError(outcome.error);
    },
    [addTiles, onError],
  );
  const fail = useCallback((reason: unknown) => onError(errorCode(reason)), [onError]);

  const attach = useCallback(() => {
    if (selected === null) return;
    api.attachPick(selected).then(apply).catch(fail);
  }, [api, selected, apply, fail]);

  const pasteFiles = useCallback(
    async (files: readonly File[]) => {
      if (selected === null) return;
      for (const file of files) {
        try {
          const bytes = new Uint8Array(await file.arrayBuffer());
          const hint = file.name === "" || GENERIC_PASTE_NAME.test(file.name) ? undefined : file.name;
          addTiles([await api.attachPaste(selected, bytes, hint)]);
          onError(null);
        } catch (reason) {
          fail(reason);
          break;
        }
      }
    },
    [api, selected, addTiles, onError, fail],
  );

  const removeTile = useCallback(
    (attachmentId: string) => {
      if (selected === null) return;
      api
        .removeDraftAttachment(selected, attachmentId)
        .then(() => dropTile(attachmentId))
        // The tile is already gone in Rust: the row catches up.
        .catch((reason: unknown) => {
          if (errorCode(reason) === "attachment_not_found") dropTile(attachmentId);
          else fail(reason);
        });
    },
    [api, selected, dropTile, fail],
  );

  // Files dragged over the window and dropped on it. The window event comes from Rust; the paths stay there.
  useEffect(() => {
    const offs: (() => void)[] = [];
    let closed = false;
    const keep = (off: () => void) => {
      if (closed) off();
      else offs.push(off);
    };
    api
      .onAttachmentsDrag((over) => setDragOver(over && latest.current.selected !== null))
      .then(keep)
      .catch(() => undefined);
    api
      .onAttachmentsDropped((token) => {
        const chat = latest.current.selected;
        // No chat open (the trash): the token is not claimed and expires in Rust.
        if (chat !== null) api.attachDropped(token, chat).then(apply).catch(fail);
      })
      .then(keep)
      .catch(() => undefined);
    return () => {
      closed = true;
      for (const off of offs) off();
    };
  }, [api, apply, fail]);

  const openImage = useCallback((entryAttachments: readonly AttachmentView[], attachmentId: string) => {
    const images = entryAttachments.filter((a) => a.kind === "image");
    const index = images.findIndex((a) => a.id === attachmentId);
    if (index >= 0) setLightbox({ images: [...images], index });
  }, []);

  const openFile = useCallback((attachmentId: string) => void api.openAttachment(attachmentId).catch(fail), [api, fail]);

  const controls: AttachmentControls = {
    tiles,
    onAttach: attach,
    onRemoveTile: removeTile,
    onPasteFiles: (files) => void pasteFiles(files),
    dragOver,
    onOpenImage: openImage,
    onOpenFile: openFile,
  };
  return {
    controls,
    lightbox,
    setLightboxIndex: (index: number) => setLightbox((was) => (was ? { ...was, index } : was)),
    closeLightbox: () => setLightbox(null),
    openFile,
  };
}
