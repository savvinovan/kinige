import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { ContextMenu, type MenuItem, type MenuPosition } from "../menu/ContextMenu";

interface AttachmentMenuProps {
  attachment: AttachmentView;
  position: MenuPosition;
  /** "Details…": the dialog opens on this attachment. */
  onDetails: () => void;
  /** "Hide as spoiler" (`true`) or "Show without spoiler" (`false`) of a photo: only the spoiler is sent. */
  onSpoiler: (spoiler: boolean) => void;
  onClose: () => void;
}

/**
 * The menu of a right-clicked photo or file card (KINIG-136): "Details…" and, for a photo, "Hide as spoiler" / "Show without spoiler", at the top.
 * The entry's own menu has "Attachment details…" for an entry with attachments.
 */
export function AttachmentMenu({ attachment, position, onDetails, onSpoiler, onClose }: AttachmentMenuProps) {
  const { t } = useTranslation();
  const items: MenuItem[] = [{ id: "details", label: t("attachments.detailsMenu"), icon: "edit" }];
  if (attachment.kind === "image") {
    items.push({
      id: "spoiler",
      label: attachment.spoiler ? t("attachments.showWithoutSpoiler") : t("attachments.hideAsSpoiler"),
      icon: "eyeOff",
    });
  }
  // The menu is drawn in a portal, but React events still bubble to the bubble this component sits in: its own menu, its keys and its drag must not
  // answer a click or a key that belongs to this one.
  const stop = (event: { stopPropagation: () => void }) => event.stopPropagation();
  return (
    <div style={{ display: "contents" }} onContextMenu={stop} onKeyDown={stop} onPointerDown={stop} onClick={stop}>
      <ContextMenu
        items={items}
        position={position}
        label={t("attachments.dialog.title")}
        onClose={onClose}
        onSelect={(id) => {
          onClose();
          if (id === "details") onDetails();
          else onSpoiler(!attachment.spoiler);
        }}
      />
    </div>
  );
}
