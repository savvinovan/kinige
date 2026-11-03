import { useCallback, useEffect, useRef } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { MessageView } from "../../api/generated/MessageView";
import type { OnThisDayEntry } from "../../api/generated/OnThisDayEntry";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import type { DraftSaver } from "../../draft/useDraftSaver";
import { focusComposer } from "./mainWindowParts";
import type { FocusTarget } from "./useFeed";

interface UseReplyActionsOptions {
  api: KinigeApi;
  /** The open chat; `null` means the trash. */
  selected: string | null;
  select: (chatId: string | null) => Promise<void>;
  focusOn: (target: FocusTarget) => void;
  draft: string;
  reply: ReplyTo | null;
  saver: DraftSaver<{ body: string; replyToId: string | null }>;
  changeReply: (reply: ReplyTo | null) => void;
  refreshReply: (reply: ReplyTo | null) => void;
  clearDraft: () => void;
  planDraft: (body: string, reply: ReplyTo | null) => void;
  appendSent: (sent: MessageView) => void;
  refreshChats: () => Promise<void>;
  setComposerError: (code: ErrorCode | null) => void;
  dismissCard: () => Promise<boolean>;
  /** The card is hidden: the feed returns to the bottom edge. */
  onDismissed: () => void;
}

/**
 * Sending an entry (including as a reply) and the slice 5 actions: "Reply" on a card entry, a jump to an entry from a
 * quote and from the card, the card's cross. No optimistic display (§5): an entry appears after Rust replies; on an
 * error the text stays in the composer with the error above it.
 */
export function useReplyActions(options: UseReplyActionsOptions) {
  const { api, selected, select, focusOn, draft, reply, saver, changeReply, refreshReply, clearDraft, planDraft, refreshChats } = options;
  const { appendSent, setComposerError, dismissCard, onDismissed } = options;
  const sending = useRef(false);
  const selectedRef = useRef(selected);
  useEffect(() => {
    selectedRef.current = selected;
  });

  /** The draft with the reply goes to Rust, and the strip above the composer takes the entry state from there. */
  const confirmReplyState = useCallback(
    async (chatId: string) => {
      await saver.flush();
      const saved = await api.getDraft(chatId).catch(() => null);
      if (saved !== null && selectedRef.current === chatId) refreshReply(saved.replyTo);
    },
    [api, saver, refreshReply],
  );

  // The reply strip shows "in the trash", "deleted" or "moved", but the entry could have been brought back (from the trash, to the same chat): on
  // window focus the state is reread. On a chat switch the draft is reread by itself (`useComposerDraft`).
  const staleReply = reply !== null && reply.state !== "live";
  useEffect(() => {
    if (!staleReply || selected === null) return;
    const onFocus = () => void confirmReplyState(selected);
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [staleReply, selected, confirmReplyState]);

  const submit = () => {
    if (sending.current || draft.trim() === "" || selected === null) return;
    sending.current = true;
    // While sending, input is not accepted (see `onDraftChange`), and the scheduled draft save is cancelled; one already
    // sent is awaited: Rust clears the chat's draft in the same transaction as the send, and a late write would
    // bring back the sent text.
    saver.cancel();
    const text = draft;
    const replyTo = reply;
    // The entry being replied to was moved to another chat: the strip above the composer already said so, the entry goes
    // without a reply (Rust accepts a reply only to an entry of the same chat).
    const replyId = replyTo === null || replyTo.state === "moved" ? null : replyTo.id;
    saver
      .settle()
      .then(() => api.sendMessage(selected, text, replyId))
      .then((sent) => {
        appendSent(sent);
        void refreshChats();
        // The person could have opened another chat while sending: Rust has already cleared the draft of the chat it was
        // sent from, and the composer, the save queue and the error now belong to the new chat and stay as they were.
        if (selectedRef.current !== selected) return;
        saver.cancel();
        clearDraft();
        setComposerError(null);
      })
      // Success: the entry simply appears in the feed, with no toast or confirmations (as in Telegram). Error: the text stays in the composer,
      // the entry is not shown; the draft returns to the save queue. If Rust said the entry being replied to
      // does not exist, the strip above the composer is reread: it will show "in the trash", "deleted" or "moved".
      .catch((reason: unknown) => {
        const code = errorCode(reason);
        // The text returns to the save queue of the previous chat (the queue is bound to it); the error line and rereading
        // the reply strip matter only for the chat that is open.
        planDraft(text, replyTo);
        if (selectedRef.current !== selected) return;
        setComposerError(code);
        if (code === "reply_target_not_found") void confirmReplyState(selected);
      })
      .finally(() => {
        sending.current = false;
      });
  };

  /** Open an entry in the feed in context (chat and entry): like a jump from search, the entry is highlighted with a fill. */
  const openEntry = (chatId: string, messageId: string) => {
    void select(chatId).then(() => focusOn({ chatId, messageId, terms: [], nonce: 0 }));
  };

  /** "Reply" on a card entry: the strip above the composer and the cursor in the composer. */
  const startReply = (entry: OnThisDayEntry) => {
    changeReply({
      id: entry.message.id,
      chatId: entry.message.chatId,
      journalDate: entry.message.journalDate,
      excerpt: entry.text,
      state: "live",
      yearsLater: 0,
    });
    focusComposer();
  };

  const hideCard = () => {
    void dismissCard().then((hidden) => {
      if (!hidden) return;
      focusComposer();
      onDismissed();
    });
  };

  return { sending, submit, openEntry, startReply, hideCard };
}
