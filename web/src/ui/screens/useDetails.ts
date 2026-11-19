import { useCallback, useState } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { AttachmentDetailsChanges } from "../../api/generated/AttachmentDetailsChanges";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import type { ErrorCode } from "../../api/generated/ErrorCode";

/** The details dialog: the attachments it can step through, the one on screen, and where the edits go (a draft tile or a sent attachment). */
export interface DetailsState {
  scope: "draft" | "sent";
  attachments: AttachmentView[];
  index: number;
  error: ErrorCode | null;
  /** The attachment whose button (a tile's "Details", a photo or a file card) gets the focus back on close: the one the dialog was opened on. */
  returnTo: string;
}

interface UseDetailsOptions {
  api: KinigeApi;
  /** The open chat's tiles above the field, and how a saved tile replaces its old self. */
  tiles: readonly AttachmentView[];
  updateTile: (view: AttachmentView) => void;
  /** A sent attachment's new view goes into the feed. */
  patchAttachment: (view: AttachmentView) => void;
  /** The error above the composer (a failed "Hide as spoiler" from the menu has no dialog to show it in). */
  onError: (code: ErrorCode | null) => void;
}

/**
 * Attachment details in the main window (KINIG-136): the dialog from a pending tile's "Details" and from the menus of an entry, and the spoiler alone
 * ("Hide as spoiler" / "Show without spoiler"). No optimistic display: the tile or the photo changes after Rust replies, and on an error the dialog
 * stays with the error and the fields as typed. A voice note has no details, so it is not in the list the dialog steps through.
 */
export function useDetails({ api, tiles, updateTile, patchAttachment, onError }: UseDetailsOptions) {
  const [state, setState] = useState<DetailsState | null>(null);

  const openDraft = useCallback(
    (attachmentId: string) => {
      const list = tiles.filter((tile) => tile.kind !== "audio");
      const index = list.findIndex((tile) => tile.id === attachmentId);
      if (index >= 0) setState({ scope: "draft", attachments: list, index, error: null, returnTo: attachmentId });
    },
    [tiles],
  );

  const openSent = useCallback((entryAttachments: readonly AttachmentView[], attachmentId: string) => {
    const list = entryAttachments.filter((attachment) => attachment.kind !== "audio");
    const index = list.findIndex((attachment) => attachment.id === attachmentId);
    if (index >= 0) setState({ scope: "sent", attachments: list, index, error: null, returnTo: attachmentId });
  }, []);

  const save = useCallback(
    async (attachmentId: string, changes: AttachmentDetailsChanges) => {
      const scope = state?.scope ?? "sent";
      try {
        const view = await (scope === "draft" ? api.setDraftAttachmentDetails(attachmentId, changes) : api.setAttachmentDetails(attachmentId, changes));
        (scope === "draft" ? updateTile : patchAttachment)(view);
        setState((was) => (was ? { ...was, error: null, attachments: was.attachments.map((a) => (a.id === view.id ? view : a)) } : was));
      } catch (reason) {
        setState((was) => (was ? { ...was, error: errorCode(reason) } : was));
        throw reason;
      }
    },
    [api, state?.scope, updateTile, patchAttachment],
  );

  /** "Hide as spoiler" / "Show without spoiler" from the context menu: only the spoiler is sent. */
  const setSpoiler = useCallback(
    (attachmentId: string, spoiler: boolean) => {
      api
        .setAttachmentDetails(attachmentId, { spoiler })
        .then(patchAttachment)
        .catch((reason: unknown) => onError(errorCode(reason)));
    },
    [api, patchAttachment, onError],
  );

  return {
    state,
    openDraft,
    openSent,
    setSpoiler,
    save,
    setIndex: (index: number) => setState((was) => (was ? { ...was, index, error: null } : was)),
    close: () => setState(null),
  };
}
