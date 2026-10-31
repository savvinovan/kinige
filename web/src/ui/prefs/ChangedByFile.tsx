import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { logTime } from "../agents/logText";
import "./prefs.css";

/**
 * «Изменено через settings.json · 14:02»: тихая строка рядом с разделом, который поменял файл (а не окно). Без `at`
 * ничего не рисует. Не тост и не подтверждение: строка стоит на месте и ничего не сдвигает.
 */
export function ChangedByFile({ at, now }: { at: number | null; now?: Date }) {
  const { t } = useTranslation();
  const language = useLanguage();
  if (at === null) return null;
  return <p className="prefs-changed">{t("prefs.changedByFile", { time: logTime(at, now ?? new Date(), language) })}</p>;
}
