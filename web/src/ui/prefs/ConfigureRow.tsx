import { useTranslation } from "react-i18next";
import { Icon } from "../icons/Icon";
import "./prefs.css";

interface ConfigureRowProps {
  /** Что настраивается: «Видимые чаты». */
  label: string;
  /** Текущее состояние тихим цветом: «2 из 5». */
  value?: string;
  onOpen: () => void;
}

/** Строка «Название · состояние · Настроить ›»: открывает подэкран. Вся строка — одна кнопка. */
export function ConfigureRow({ label, value, onOpen }: ConfigureRowProps) {
  const { t } = useTranslation();
  return (
    <button type="button" className="configure-row" onClick={onOpen}>
      <span className="configure-row__text">
        <span className="configure-row__label">{label}</span>
        {value ? <span className="configure-row__value">{value}</span> : null}
      </span>
      <span className="configure-row__action">
        {t("prefs.configure")}
        <Icon name="chevronRight" />
      </span>
    </button>
  );
}
