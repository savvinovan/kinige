import { useTranslation } from "react-i18next";
import "./recovery.css";

/**
 * The "База создана более новой версией Kinige" screen (`StartupState.databaseTooNew`, §10.4): the whole window, no buttons.
 * Downgrading is not supported and copies will not help: the app must be updated. Nothing was changed or deleted.
 * The look is the same as for "База повреждена" (a centered column), but the cause is different, hence a separate component.
 */
export function TooNewScreen() {
  const { t } = useTranslation();
  return (
    <main className="recovery" aria-labelledby="too-new-title">
      <div className="recovery__column">
        <h1 className="recovery__title" id="too-new-title">
          {t("recovery.tooNewTitle")}
        </h1>
        <p className="recovery__text">{t("recovery.tooNewText")}</p>
      </div>
    </main>
  );
}
