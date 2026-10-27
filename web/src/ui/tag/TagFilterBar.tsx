import { useTranslation } from "react-i18next";
import { IconButton } from "../controls/IconButton";
import "./tag.css";

interface TagFilterBarProps {
  /** Тег фильтра в нижнем регистре, без `#`. */
  tag: string;
  /** Сбросить фильтр (кнопка «×»). Esc обрабатывает родитель. */
  onClear: () => void;
}

// Подстановка для разбора фразы: тег в ней выделяется отдельным элементом, а порядок слов остаётся за переводом.
const SLOT = "\u0000";

/**
 * Строка над лентой при фильтре по тегу: «Показаны записи с #тег» тихо, тег акцентным цветом, справа кнопка «×»
 * («Сбросить фильтр»). Длинный тег переносится, строка не растёт за колонку ленты.
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
