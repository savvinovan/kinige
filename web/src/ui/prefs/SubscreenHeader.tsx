import { useTranslation } from "react-i18next";
import { Icon } from "../icons/Icon";
import "./prefs.css";

interface SubscreenHeaderProps {
  /** Раздел, из которого открыт подэкран: «Доступ ИИ-агентов». */
  parent: string;
  /** Название подэкрана: «Видимые чаты». */
  title: string;
  onBack: () => void;
}

/**
 * Заголовок подэкрана: стрелка назад и строка «Раздел › Название». Стрелка — настоящая кнопка с подписью «Назад: раздел».
 * Название подэкрана — заголовок страницы (`h2`), раздел перед ним — тихий текст.
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
