import { useTranslation } from "react-i18next";
import "./recovery.css";

/**
 * Экран «База создана более новой версией Kinige» (`StartupState.databaseTooNew`, §10.4): во всё окно, без кнопок.
 * Откат версии не поддерживается, копии не помогут: нужно обновить приложение. Ничего не изменено и не удалено.
 * Вид тот же, что у «База повреждена» (колонка по центру), но это другая причина, поэтому отдельный компонент.
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
