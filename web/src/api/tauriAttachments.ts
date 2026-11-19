import { invoke } from "@tauri-apps/api/core";
import type { AttachmentDetailsChanges } from "./generated/AttachmentDetailsChanges";
import type { AttachmentView } from "./generated/AttachmentView";
import { call, toApiError } from "./tauriCall";

// Attachment commands of the main window that need more than one line (slices 7 and 7b), apart from `TauriApi` to keep that file short.

/** `attach_paste`: a raw body; the chat id and the name go in headers (ASCII, so the name is percent-encoded; Rust decodes and cleans it). */
export async function attachPaste(chatId: string, bytes: Uint8Array, nameHint?: string): Promise<AttachmentView> {
  const headers: Record<string, string> = { "x-chat-id": chatId };
  if (nameHint) headers["x-name-hint"] = encodeURIComponent(nameHint);
  try {
    return await invoke<AttachmentView>("attach_paste", bytes, { headers });
  } catch (reason) {
    throw toApiError(reason);
  }
}

/** `set_draft_attachment_details`: the changes are sent as they are, an absent key meaning "leave" and `null` meaning "clear". */
export function setDraftAttachmentDetails(attachmentId: string, changes: AttachmentDetailsChanges): Promise<AttachmentView> {
  return call("set_draft_attachment_details", { attachmentId, changes });
}

/** `set_attachment_details`: the same shape for a sent attachment. */
export function setAttachmentDetails(attachmentId: string, changes: AttachmentDetailsChanges): Promise<AttachmentView> {
  return call("set_attachment_details", { attachmentId, changes });
}
