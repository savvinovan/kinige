import { useState } from "react";
import { INBOX_ID, type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { ChatAction } from "../sidebar/Sidebar";
import { type ChatDialogState, focusComposer } from "./mainWindowParts";

interface UseChatDialogOptions {
  api: KinigeApi;
  /** The open chat; `null` means the trash. */
  selected: string | null;
  select: (chatId: string | null) => Promise<void>;
  refreshChats: () => Promise<void>;
  /** An emoji was used in the dialog: it goes to the recent ones. */
  recordEmoji: (emoji: string) => void;
  /** An action error that has no place in the dialog (shown above the composer). */
  onError: (code: ErrorCode) => void;
}

/**
 * The "New chat" / "Rename" / "Chat emoji" dialog and the actions of the sidebar chat menu (pin, archive, colour).
 * No optimistic display: the sidebar changes after Rust replies.
 */
export function useChatDialog({ api, selected, select, refreshChats, recordEmoji, onError }: UseChatDialogOptions) {
  const [dialog, setDialog] = useState<ChatDialogState | null>(null);
  const [dialogError, setDialogError] = useState<ErrorCode | null>(null);

  const closeDialog = () => {
    setDialog(null);
    setDialogError(null);
  };

  const submitDialog = (name: string, emoji: string | null) => {
    if (!dialog) return;
    setDialogError(null);
    const request =
      dialog.mode === "create"
        ? api.createChat(name, emoji)
        : dialog.mode === "rename"
          ? api.renameChat(dialog.chatId, name)
          : api.setChatIcon(dialog.chatId, emoji ?? "");
    request
      .then(async (chat) => {
        if (emoji) recordEmoji(emoji);
        closeDialog();
        await refreshChats();
        if (dialog.mode === "create") {
          await select(chat.id);
          focusComposer();
        }
      })
      .catch((reason: unknown) => setDialogError(errorCode(reason)));
  };

  const chatAction = (id: string, action: ChatAction) => {
    if (action === "rename" || action === "icon") {
      setDialog({ mode: action, chatId: id });
      return;
    }
    const request =
      action === "pin" || action === "unpin"
        ? api.setChatPinned(id, action === "pin")
        : api.setChatArchived(id, action === "archive");
    request
      .then(async () => {
        await refreshChats();
        // An archived chat is closed: Inbox stays open.
        if (action === "archive" && selected === id) await select(INBOX_ID);
      })
      .catch((reason: unknown) => onError(errorCode(reason)));
  };

  return { dialog, dialogError, openCreate: () => setDialog({ mode: "create" }), closeDialog, submitDialog, chatAction };
}
