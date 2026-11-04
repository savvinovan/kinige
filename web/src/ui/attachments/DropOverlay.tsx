import { useTranslation } from "react-i18next";
import "./drop-overlay.css";

/**
 * "Drop to attach" over the feed while a file is dragged over the window (slice 7). Only a picture of the state: a quiet accent
 * outline and a label. The drop itself is taken by the native window, so the overlay lets every pointer event through
 * (`pointer-events: none`). It fills the nearest positioned ancestor; no animation. When it is not visible it is hidden from
 * the screen reader as well.
 */
export function DropOverlay({ visible }: { visible: boolean }) {
  const { t } = useTranslation();
  return (
    <div className={visible ? "drop-overlay drop-overlay--visible" : "drop-overlay"} aria-hidden={visible ? undefined : true}>
      <div className="drop-overlay__outline">
        <span className="drop-overlay__label">{t("attachments.dropToAttach")}</span>
      </div>
    </div>
  );
}
