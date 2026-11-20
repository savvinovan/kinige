import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ContextMenu, type MenuItem, type MenuPosition } from "../menu/ContextMenu";
import { ChatTile } from "../sidebar/ChatTile";
import type { MoveTarget } from "./moveTargets";

interface MessageMenuProps {
  position: MenuPosition;
  isTask: boolean;
  favorite: boolean;
  /** Read-only entry: all items except "Copy" are disabled. */
  readOnly: boolean;
  /** No handler - the item is visible but disabled. */
  onEdit?: () => void;
  /** An entry that has no text to edit (a sticker, slice 5c): "Edit" is not in the menu at all, not a disabled item. */
  noEdit?: boolean;
  onToggleTask?: () => void;
  onToggleFavorite?: () => void;
  onChangeDate?: () => void;
  onDelete?: () => void;
  /** "Attachment details…" (KINIG-136): shown only for an entry with attachments that have details (not for a voice note alone). */
  onAttachmentDetails?: () => void;
  /** "Add reaction…" (slice 5a): opens the quick popover at the menu's place. Absent for an entry that cannot take reactions from here. */
  onAddReaction?: () => void;
  /** Why "Add reaction…" cannot be used (10 live reactions): the item is shut and says so in its tooltip. */
  addReactionReason?: string;
  /** Where the entry can be moved (the "Move to chat" submenu) and what to do on choice; without them the item is disabled. */
  moveTargets?: readonly MoveTarget[];
  onMoveTo?: (chatId: string) => void;
  onCopy: () => void;
  onClose: () => void;
}

const CHAT_PREFIX = "chat:";

/**
 * Entry context menu: "Copy", "Edit", "Task", "Favorite", "Change date...", "Move to chat"
 * (a submenu with the list of chats), "Delete" (dangerous, below a divider). The submenu is a second level in the same panel: "Back", then
 * pinned, regular and archived chats; the current chat is disabled. Arrows and Enter move and choose, Esc and the left arrow
 * return to the first level.
 */
export function MessageMenu({
  position,
  isTask,
  favorite,
  readOnly,
  onEdit,
  noEdit = false,
  onToggleTask,
  onToggleFavorite,
  onChangeDate,
  onDelete,
  onAttachmentDetails,
  onAddReaction,
  addReactionReason,
  moveTargets = [],
  onMoveTo,
  onCopy,
  onClose,
}: MessageMenuProps) {
  const { t } = useTranslation();
  const [level, setLevel] = useState<"main" | "move">("main");
  const off = (handler: (() => void) | undefined) => readOnly || !handler;
  const canMove = !readOnly && onMoveTo !== undefined && moveTargets.some((target) => !target.current);

  const main: MenuItem[] = [
    { id: "copy", label: t("message.copy"), icon: "copy" },
    ...(onAddReaction
      ? [{ id: "react", label: t("message.addReaction"), icon: "reactionAdd" as const, disabled: readOnly || addReactionReason !== undefined, hint: addReactionReason }]
      : []),
    ...(noEdit ? [] : [{ id: "edit", label: t("message.edit"), icon: "edit" as const, disabled: off(onEdit) }]),
    { id: "task", label: isTask ? t("message.untask") : t("message.task"), icon: "task", disabled: off(onToggleTask) },
    {
      id: "favorite",
      label: favorite ? t("message.unfavorite") : t("message.favorite"),
      icon: "favorite",
      disabled: off(onToggleFavorite),
    },
    { id: "date", label: t("message.changeDate"), icon: "calendar", disabled: off(onChangeDate) },
    { id: "move", label: t("message.moveTo"), icon: "moveTo", disabled: !canMove },
    ...(onAttachmentDetails ? [{ id: "attachments", label: t("attachments.detailsEntryMenu"), icon: "edit" as const, disabled: readOnly }] : []),
    { id: "delete", label: t("message.delete"), icon: "trash", danger: true, separatorBefore: true, disabled: off(onDelete) },
  ];

  const submenu: MenuItem[] = [
    { id: "back", label: t("message.moveBack"), icon: "chevronLeft" },
    ...moveTargets.map((target, index) => ({
      id: `${CHAT_PREFIX}${target.id}`,
      label: target.name,
      // The chat as the sidebar shows it: the emoji, or the letter on the chat's colour.
      tile: <ChatTile chat={target} name={target.name} />,
      disabled: target.current,
      // A thin divider between sections (pinned / chats / archive) and under "Back".
      separatorBefore: index === 0 || target.group !== moveTargets[index - 1]?.group,
    })),
  ];

  const handlers: Record<string, (() => void) | undefined> = {
    copy: onCopy,
    edit: onEdit,
    task: onToggleTask,
    favorite: onToggleFavorite,
    date: onChangeDate,
    delete: onDelete,
    attachments: onAttachmentDetails,
    react: onAddReaction,
  };

  return (
    <ContextMenu
      items={level === "main" ? main : submenu}
      position={position}
      label={level === "main" ? t("message.menu") : t("message.moveMenu")}
      levelKey={level}
      onBack={level === "move" ? () => setLevel("main") : undefined}
      onClose={onClose}
      onSelect={(id) => {
        if (id === "move") {
          setLevel("move");
          return;
        }
        if (id === "back") {
          setLevel("main");
          return;
        }
        onClose();
        if (id.startsWith(CHAT_PREFIX)) onMoveTo?.(id.slice(CHAT_PREFIX.length));
        else handlers[id]?.();
      }}
    />
  );
}
