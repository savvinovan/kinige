import { memo, useMemo, type PointerEvent } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { MessageAttachments } from "../attachments/MessageAttachments";
import { useReveal } from "../attachments/reveal";
import { Message } from "../message/Message";
import type { MoveTarget } from "../message/moveTargets";
import { ReplyQuote } from "../reply/ReplyQuote";
import { showsWaveform } from "../voice/VoicePlayer";
import type { FeedMessage } from "./buildFeed";
import type { FeedActions } from "./Feed";

interface FeedRowProps {
  message: FeedMessage;
  /** Continues a series: the gap above is smaller. */
  inSeries: boolean;
  /** The entry opened from search: highlighted with a fill, `markTerms` are marked in the text. */
  marked: boolean;
  markTerms?: readonly string[];
  mode: "view" | "editing" | "deleted";
  /** The error of saving an edit; only for the entry being edited. */
  editError?: string;
  onRetry?: (messageId: string) => void;
  onOpenLink?: (url: string) => void;
  onOpenReply?: (quote: ReplyTo) => void;
  onOpenImage?: (entryAttachments: readonly AttachmentView[], attachmentId: string) => void;
  onOpenFile?: (attachmentId: string) => void;
  /** KINIG-136: the details dialog on an attachment of the entry (the entry's attachments are given, for the arrows of the dialog), and the spoiler alone. */
  onAttachmentDetails?: (entryAttachments: readonly AttachmentView[], attachmentId: string) => void;
  onAttachmentSpoiler?: (attachmentId: string, spoiler: boolean) => void;
  actions?: FeedActions;
  moveTargets?: readonly MoveTarget[];
  onDragStart?: (messageId: string) => (event: PointerEvent<HTMLElement>, plate: HTMLElement) => void;
}

/**
 * One entry of the feed. Every prop is a value or a function that stays the same while the entry itself does not change,
 * and the row is memoised: sending an entry, loading a page or a keystroke elsewhere does not redraw the entries around it (KINIG-47).
 */
export const FeedRow = memo(function FeedRow({
  message,
  inSeries,
  marked,
  markTerms,
  mode,
  editError,
  onRetry,
  onOpenLink,
  onOpenReply,
  onOpenImage,
  onOpenFile,
  onAttachmentDetails,
  onAttachmentSpoiler,
  actions,
  moveTargets,
  onDragStart,
}: FeedRowProps) {
  const { t } = useTranslation();
  const { isRevealed } = useReveal();
  const id = message.id;
  const sentAt = useMemo(() => new Date(message.createdAt), [message.createdAt]);
  const retry = useMemo(() => (onRetry ? () => onRetry(id) : undefined), [onRetry, id]);
  const handlers = useMemo(() => (actions ? messageHandlers(actions, id) : undefined), [actions, id]);
  const dragStart = useMemo(() => onDragStart?.(id), [onDragStart, id]);
  const quote = useMemo(
    () => (message.replyTo ? <ReplyQuote quote={message.replyTo} onOpen={onOpenReply} /> : undefined),
    [message.replyTo, onOpenReply],
  );
  const entryAttachments = message.attachments;
  const attachments = useMemo(
    () =>
      entryAttachments && entryAttachments.length > 0 ? (
        <MessageAttachments
          attachments={entryAttachments}
          onOpenImage={(attachmentId) => onOpenImage?.(entryAttachments, attachmentId)}
          onOpenFile={(attachmentId) => onOpenFile?.(attachmentId)}
          onDetails={onAttachmentDetails ? (attachmentId) => onAttachmentDetails(entryAttachments, attachmentId) : undefined}
          onSpoiler={onAttachmentSpoiler}
        />
      ) : undefined,
    [entryAttachments, onOpenImage, onOpenFile, onAttachmentDetails, onAttachmentSpoiler],
  );
  // KINIG-136: "Attachment details…" in the entry's menu, on the first attachment that has details (a voice note has none).
  const detailsTarget = entryAttachments?.find((attachment) => attachment.kind !== "audio");
  const openDetails = useMemo(
    () => (onAttachmentDetails && entryAttachments && detailsTarget ? () => onAttachmentDetails(entryAttachments, detailsTarget.id) : undefined),
    [onAttachmentDetails, entryAttachments, detailsTarget],
  );
  // A single photo with a description and no text: the description is the entry's one line; a covered photo says "Description hidden" there.
  const only = entryAttachments?.length === 1 && entryAttachments[0]?.kind === "image" ? entryAttachments[0] : undefined;
  const caption =
    only?.description && message.bodyMd.trim() === ""
      ? only.spoiler && !isRevealed(only.id)
        ? { text: t("attachments.descriptionHidden"), muted: true }
        : { text: only.description, muted: false }
      : undefined;
  // A voice note with no text ends on the player's meta line, which holds the room for the time (slice 7a).
  const lastAttachment = entryAttachments?.at(-1);
  const timeBeside = message.bodyMd.trim() === "" && lastAttachment !== undefined && showsWaveform(lastAttachment);
  // Pictures only: the album fills the bubble edge to edge (KINIG-132).
  const imagesOnly = entryAttachments !== undefined && entryAttachments.length > 0 && entryAttachments.every(({ kind }) => kind === "image");

  return (
    <li
      className={`feed__item${inSeries ? " feed__item--series" : ""}`}
      data-message-id={id}
      data-journal-date={message.journalDate}
    >
      <Message
        markdown={message.bodyMd}
        sentAt={sentAt}
        timeZone={message.createdTz}
        status={message.status}
        highlighted={message.highlighted || marked}
        markTerms={marked ? markTerms : undefined}
        onRetry={retry}
        onOpenLink={onOpenLink}
        isTask={message.isTask}
        taskDone={message.taskDoneAt != null}
        favorite={message.favoriteAt != null}
        byAgent={message.origin === "agent"}
        readOnly={message.readOnly}
        quote={quote}
        attachments={attachments}
        timeBeside={timeBeside}
        imagesOnly={imagesOnly}
        caption={imagesOnly ? caption : undefined}
        onAttachmentDetails={openDetails}
        reactions={message.reactions}
        reactionKeysHad={message.reactionKeysHad}
        tags={message.tags}
        mode={mode}
        editError={mode === "editing" ? editError : undefined}
        moveTargets={moveTargets}
        onDragStart={dragStart}
        {...handlers}
      />
    </li>
  );
});

/** Bubble handlers by entry id. */
function messageHandlers(actions: FeedActions, id: string) {
  return {
    onEdit: () => actions.onEdit(id),
    onSaveEdit: (text: string) => actions.onSaveEdit(id, text),
    onCancelEdit: () => actions.onCancelEdit(id),
    onToggleTask: (on: boolean) => actions.onToggleTask(id, on),
    onToggleDone: (done: boolean) => actions.onToggleDone(id, done),
    onToggleFavorite: (on: boolean) => actions.onToggleFavorite(id, on),
    onReact: actions.onReact ? (emoji: string, on: boolean) => actions.onReact?.(id, emoji, on) : undefined,
    onChangeDate: () => actions.onChangeDate(id),
    onDelete: () => actions.onDelete(id),
    onRestore: () => actions.onRestore(id),
    onToggleTaskItem: (index: number, checked: boolean) => actions.onToggleTaskItem(id, index, checked),
    onTagClick: actions.onTagClick,
    onMoveTo: (chatId: string) => actions.onMoveTo(id, chatId),
  };
}
