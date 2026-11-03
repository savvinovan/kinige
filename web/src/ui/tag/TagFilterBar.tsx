import { useTranslation } from "react-i18next";
import { IconButton } from "../controls/IconButton";
import "./tag.css";

interface TagFilterBarProps {
  /** The filter tag in lower case, without `#`. */
  tag: string;
  /** Clear the filter (the "×" button). Esc is handled by the parent. */
  onClear: () => void;
}

// A placeholder for splitting the phrase: the tag in it is rendered as a separate element, while the word order is left to the translation.
const SLOT = "\u0000";

/**
 * The row above the feed when filtering by a tag: "Показаны записи с #тег" (entries with #tag shown) quietly, the tag in the accent color, a "×" button on the right
 * ("Clear filter"). A long tag wraps, the row does not grow beyond the feed column.
 */
export function TagFilterBar({ tag, onClear }: TagFilterBarProps) {
  const { t } = useTranslation();
  const [before = "", after = ""] = t("tag.filterShown", { tag: SLOT }).split(SLOT);
  return (
    <div className="tag-filter" role="region" aria-label={t("tag.filterLabel")}>
      <div className="tag-filter__inner">
        <p className="tag-filter__text">
          {before}
          <span className="tag-filter__tag">#{tag}</span>
          {after}
        </p>
        <IconButton small icon="close" label={t("tag.clear")} onClick={onClear} />
      </div>
    </div>
  );
}
