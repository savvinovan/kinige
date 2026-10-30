import { Fragment, useId } from "react";
import { useTranslation } from "react-i18next";
import type { SettingsFileState } from "../../../api/generated/SettingsFileState";
import { Button } from "../../controls/Button";
import { useCopy } from "../../message/useCopy";
import { StatusNote } from "../../settings/StatusNote";
import "../prefs.css";
import "../../settings/settings.css";

// Разрыв строки предлагается после каждого разделителя пути; запасной перенос в любом месте — в CSS (`path-row__path`).
const segments = (path: string) => path.split(/(?<=[\\/])/);

/**
 * «Файл настроек»: путь с кнопкой «Копировать путь» и состояние файла. Файл без ошибок — тихая строка; файл не разобран —
 * ошибка с позицией («строка 1, столбец 15»), остальные настройки прежние; небезопасный файл — причина, по которой ключи
 * доступа и папки из него не применяются; ошибки ключей — по строке на ключ; неизвестные ключи — предупреждение (в файле они
 * остаются). Тостов нет: всё это стоит на странице и ничего не двигает.
 */
export function FilePage({ file }: { file: SettingsFileState }) {
  const { t } = useTranslation();
  const titleId = useId();
  const pathId = useId();
  const { state, copy } = useCopy();
  const copyLabel = state === "copied" ? t("prefs.file.copied") : state === "failed" ? t("prefs.file.copyFailed") : t("prefs.file.copyPath");
  const { problem, unsafeReason } = file;
  const clean = !problem && !unsafeReason && file.issues.length === 0;
  return (
    <section className="prefs-page" aria-labelledby={titleId}>
      <h2 className="prefs-page__title" id={titleId}>
        {t("prefs.section.file")}
      </h2>
      <div className="prefs-block">
        <p className="settings__description">{t("prefs.file.description")}</p>
        <div className="path-row" role="group" aria-labelledby={`${pathId}-label`}>
          <span className="path-row__label" id={`${pathId}-label`}>
            {t("prefs.file.pathLabel")}
          </span>
          <div className="path-row__line">
            <div className="path-row__field">
              <code className="path-row__path" id={pathId} title={file.path}>
                {segments(file.path).map((part, index) => (
                  <Fragment key={index}>
                    {part}
                    <wbr />
                  </Fragment>
                ))}
              </code>
            </div>
            <Button
              variant="secondary"
              aria-label={state === "idle" ? t("prefs.file.copyPathLabel") : undefined}
              aria-describedby={pathId}
              onClick={() => void copy(file.path)}
            >
              {copyLabel}
            </Button>
          </div>
        </div>
        {problem ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {t(`prefs.file.problem.${problem.kind}`, { line: problem.line ?? 0, column: problem.column ?? 0 })}
          </StatusNote>
        ) : null}
        {unsafeReason ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {t(`prefs.file.unsafe.${unsafeReason}`)}
          </StatusNote>
        ) : null}
        {clean ? (
          <StatusNote tone="quiet" icon="done" role="status">
            {t("prefs.file.ok")}
          </StatusNote>
        ) : null}
      </div>
      {file.issues.length > 0 ? (
        <div className="prefs-block">
          <h3 className="prefs-file__heading">{t("prefs.file.problemsTitle")}</h3>
          <ul className="prefs-file" aria-label={t("prefs.file.problemsTitle")}>
            {file.issues.map((issue) => (
              <li key={`${issue.key}:${issue.kind}`} className="prefs-file__item">
                <StatusNote tone="error" icon="alert">
                  {t(`prefs.file.issue.${issue.kind}`, { key: issue.key })}
                </StatusNote>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {file.unknownKeys.length > 0 ? (
        <div className="prefs-block">
          <h3 className="prefs-file__heading">{t("prefs.file.unknownTitle")}</h3>
          <ul className="prefs-file" aria-label={t("prefs.file.unknownTitle")}>
            {file.unknownKeys.map((key) => (
              <li key={key} className="prefs-file__item">
                <StatusNote tone="quiet" icon="info">
                  {t("prefs.file.unknownKey", { key })}
                </StatusNote>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
