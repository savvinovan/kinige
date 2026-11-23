import { useTranslation } from "react-i18next";
import "./gif-badge.css";

/** The "GIF" label on the corner of a still frame. The cell it sits on carries the accessible name; the badge itself is not read twice. */
export function GifBadge() {
  const { t } = useTranslation();
  return (
    <span className="gif-badge" aria-hidden="true">
      <span>{t("stickers.gifs.badge")}</span>
    </span>
  );
}
