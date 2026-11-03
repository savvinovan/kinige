import { useTranslation } from "react-i18next";
import { Icon } from "../icons/Icon";
import "./prefs.css";

interface SubscreenHeaderProps {
  /** The section the subscreen was opened from: "Доступ ИИ-агентов". */
  parent: string;
  /** The subscreen name: "Видимые чаты". */
  title: string;
  onBack: () => void;
}

/**
 * The subscreen header: a back arrow and a "Section > Name" line. The arrow is a real button labeled "Назад: раздел".
 * The subscreen name is the page heading (`h2`), the section before it is quiet text.
 */
export function SubscreenHeader({ parent, title, onBack }: SubscreenHeaderProps) {
  const { t } = useTranslation();
  return (
    <header className="prefs-subscreen">
      <button type="button" className="prefs-subscreen__back" aria-label={t("prefs.backTo", { section: parent })} onClick={onBack}>
        <Icon name="chevronLeft" />
      </button>
      <div className="prefs-subscreen__path">
        <span className="prefs-subscreen__parent">{parent}</span>
        <span className="prefs-subscreen__arrow">
          <Icon name="chevronRight" />
        </span>
        <h2 className="prefs-subscreen__title">{title}</h2>
      </div>
    </header>
  );
}
