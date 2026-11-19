import { useTranslation } from "react-i18next";
import { Icon } from "../icons/Icon";
import "./spoiler.css";

/**
 * The cover of a hidden photo (KINIG-136): a flat quiet shape that fills the photo's own box, with a crossed-out eye and "Hidden" (only the
 * eye in a cell or tile too small for the word). It draws nothing of the picture and loads nothing: no `img`, no request. It has no hover state,
 * so moving the pointer over it changes nothing. It is only the face: the button around it (the cell, the tile) reveals the photo.
 */
export function SpoilerFace({ compact = false }: { compact?: boolean }) {
  const { t } = useTranslation();
  return (
    <span className={compact ? "spoiler-face spoiler-face--compact" : "spoiler-face"} data-spoiler-face="">
      <Icon name="eyeOff" />
      {compact ? null : <span className="spoiler-face__text">{t("attachments.hidden")}</span>}
    </span>
  );
}

/**
 * "Hide" on a revealed photo: a small pill in the corner that shows on hover and on keyboard focus of the picture (it is in the tab order all
 * the time, only its fill is transparent until then, so nothing around it moves). It is a sibling of the picture's button, never inside it.
 */
export function HideButton({ onHide, className }: { onHide: () => void; className?: string }) {
  const { t } = useTranslation();
  return (
    <button type="button" className={className ? `spoiler-hide ${className}` : "spoiler-hide"} aria-label={t("attachments.hideLabel")} onClick={onHide}>
      <span className="spoiler-hide__text">{t("attachments.hide")}</span>
    </button>
  );
}
