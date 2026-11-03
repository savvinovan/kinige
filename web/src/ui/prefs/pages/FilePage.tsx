import { Fragment, useId } from "react";
import { useTranslation } from "react-i18next";
import type { SettingsFileState } from "../../../api/generated/SettingsFileState";
import { CopyButton } from "../../controls/CopyButton";
import { StatusNote } from "../../settings/StatusNote";
import "../prefs.css";
import "../../settings/settings.css";

// A line break is offered after each path separator; the fallback wrap at any position is in CSS (`path-row__path`).
const segments = (path: string) => path.split(/(?<=[\\/])/);

/**
 * "Файл настроек": the path with the "Копировать путь" button and the file state. A file without errors is a quiet line; a file that
 * did not parse is an error with a position ("строка 1, столбец 15"), the other settings stay as they were; an unsafe file is the reason why the access
 * keys and folders from it are not applied; key errors are one line per key; unknown keys are a warning (they
 * stay in the file). There are no toasts: all of this stands on the page and moves nothing.
 */
export function FilePage({ file }: { file: SettingsFileState }) {
  const { t } = useTranslation();
  const titleId = useId();
  const pathId = useId();
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
            <CopyButton
              text={file.path}
              label={t("prefs.file.copyPath")}
              copiedLabel={t("prefs.file.copied")}
              failedLabel={t("prefs.file.copyFailed")}
              describedBy={pathId}
            />
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
            {file.issues.map((issue, index) => (
              <li key={`${index}:${issue.key}:${issue.kind}`} className="prefs-file__item">
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
            {file.unknownKeys.map((key, index) => (
              <li key={`${index}:${key}`} className="prefs-file__item">
                <StatusNote tone="quiet" icon="info">
                  {t("prefs.file.unknownKey", { key })}
                </StatusNote>
              </li>
            ))}
            {file.unknownMore > 0 ? (
              <li className="prefs-file__item">
                <StatusNote tone="quiet" icon="info">
                  {t("prefs.file.unknownMore", { count: file.unknownMore })}
                </StatusNote>
              </li>
            ) : null}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
