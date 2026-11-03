import { Fragment, useId, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../controls/Button";
import "./settings.css";

interface PathRowProps {
  /** The folder path; `null` means no folder is chosen. */
  path: string | null;
  /** The default folder: a badge "Папка по умолчанию" stands next to the path. */
  isDefault?: boolean;
  /** The system folder picker dialog is open: "Выбрать…" is announced as unavailable and cannot be pressed again. */
  busy?: boolean;
  onChoose: () => void;
}

// A line break is offered after each path separator; the fallback wrap at any position (overflow-wrap) is in CSS.
const segments = (path: string) => path.split(/(?<=[\\/])/);

/**
 * The "Папка" row: a label, the path, the "Папка по умолчанию" badge and the "Выбрать…" button.
 *
 * A long path **wraps** instead of being truncated: for a path it matters to see both the beginning (where it lies) and the end (what it is called), and
 * truncation with an ellipsis in the middle is impossible in plain CSS, while truncation from the start (`direction: rtl`) breaks the order of `~` and `/`.
 * The wrap goes at path separators, and a folder name that is too long wraps anywhere. The whole path is in `title`.
 *
 * "Выбрать…" is a real button with visible text. While the dialog is open (`busy`), it stays in the Tab order with `aria-disabled`
 * and `aria-busy`, but pressing it does nothing.
 */
export function PathRow({ path, isDefault = false, busy = false, onChoose }: PathRowProps) {
  const { t } = useTranslation();
  const labelId = useId();
  const pathId = useId();

  const onClick = (event: MouseEvent<HTMLButtonElement>) => {
    if (busy) event.preventDefault();
    else onChoose();
  };

  return (
    <div className="path-row" role="group" aria-labelledby={labelId}>
      <span className="path-row__label" id={labelId}>
        {t("settings.mirror.folder")}
      </span>
      <div className="path-row__line">
        <div className="path-row__field">
          {path === null ? (
            <span className="path-row__empty" id={pathId}>
              {t("settings.mirror.notSelected")}
            </span>
          ) : (
            <code className="path-row__path" id={pathId} title={path}>
              {segments(path).map((part, index) => (
                <Fragment key={index}>
                  {part}
                  <wbr />
                </Fragment>
              ))}
            </code>
          )}
          {isDefault && path !== null ? <span className="path-row__badge">{t("settings.mirror.isDefault")}</span> : null}
        </div>
        <Button variant="secondary" aria-disabled={busy || undefined} aria-busy={busy || undefined} aria-describedby={pathId} onClick={onClick}>
          {t("settings.mirror.choose")}
        </Button>
      </div>
    </div>
  );
}
