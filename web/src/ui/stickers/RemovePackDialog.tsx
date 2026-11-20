import { useState } from "react";
import { useTranslation } from "react-i18next";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { StickerPackView } from "../../api/generated/StickerPackView";
import { Button } from "../controls/Button";
import { Modal } from "../settings/Modal";
import { StatusNote } from "../settings/StatusNote";
import { packTitle } from "./packTitle";
import "./add-pack.css";

interface RemovePackDialogProps {
  pack: Pick<StickerPackView, "source" | "title">;
  error?: ErrorCode | null;
  onConfirm: () => void;
  onClose: () => void;
}

/** "Remove «Test Pack»? The stickers you sent stay in your entries." (slice 5c, the shared `Modal`). Esc and Cancel close it and remove nothing. */
export function RemovePackDialog({ pack, error, onConfirm, onClose }: RemovePackDialogProps) {
  const { t } = useTranslation();
  return (
    <Modal title={t("stickers.remove.title")} closeLabel={t("common.close")} onClose={onClose}>
      <div className="add-pack">
        <p className="add-pack__text">{t("stickers.remove.question", { title: packTitle(pack, t) })}</p>
        {error ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {t(`error.${error}`)}
          </StatusNote>
        ) : null}
        <div className="add-pack__actions">
          <Button variant="accent" onClick={onClose}>
            {t("stickers.remove.cancel")}
          </Button>
          <Button variant="secondary" onClick={onConfirm}>
            {t("stickers.remove.confirm")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

/** The question on the real `KinigeApi`: "Remove" asks Rust, the pack and its files go and the sent stickers stay; an error stays in the dialog. */
export function RemovePackHost({ api, pack, onClose }: { api: KinigeApi; pack: StickerPackView; onClose: () => void }) {
  const [error, setError] = useState<ErrorCode | null>(null);
  return (
    <RemovePackDialog
      pack={pack}
      error={error}
      onClose={onClose}
      onConfirm={() => {
        api.removeStickerPack(pack.id).then(onClose, (reason: unknown) => {
          // The pack is already gone (removed elsewhere): the question has nothing left to do.
          if (errorCode(reason) === "sticker_not_found") onClose();
          else setError(errorCode(reason));
        });
      }}
    />
  );
}
