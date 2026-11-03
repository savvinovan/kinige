import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { logTime } from "../agents/logText";
import "./prefs.css";

/**
 * "Изменено через settings.json · 14:02": a quiet line next to the section that the file changed (not the window). Without `at`
 * it draws nothing. Not a toast and not a confirmation: the line stays in place and shifts nothing.
 */
export function ChangedByFile({ at, now }: { at: number | null; now?: Date }) {
  const { t } = useTranslation();
  const language = useLanguage();
  if (at === null) return null;
  return <p className="prefs-changed">{t("prefs.changedByFile", { time: logTime(at, now ?? new Date(), language) })}</p>;
}
