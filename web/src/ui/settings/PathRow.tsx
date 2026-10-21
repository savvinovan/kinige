import { Fragment, useId, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../controls/Button";
import "./settings.css";

interface PathRowProps {
  /** Путь папки; `null` — папка не выбрана. */
  path: string | null;
  /** Папка по умолчанию: рядом с путём стоит значок-подпись «Папка по умолчанию». */
  isDefault?: boolean;
  /** Системный диалог выбора папки открыт: «Выбрать…» объявлена недоступной и повторно не нажимается. */
  busy?: boolean;
  onChoose: () => void;
}

// Разрыв строки предлагается после каждого разделителя пути; запасной перенос в любом месте (overflow-wrap) — в CSS.
const segments = (path: string) => path.split(/(?<=[\\/])/);

/**
 * Строка «Папка»: подпись, путь, значок «Папка по умолчанию» и кнопка «Выбрать…».
 *
 * Длинный путь **переносится**, а не обрезается: по пути важно видеть и начало (где лежит), и конец (как называется), а
 * обрезка многоточием посередине невозможна на чистом CSS, обрезка с начала (`direction: rtl`) ломает порядок знаков `~` и `/`.
 * Перенос идёт по разделителям пути, а слишком длинное имя папки — в любом месте. Путь целиком — в `title`.
 *
 * «Выбрать…» — настоящая кнопка с видимым текстом. Пока диалог открыт (`busy`), она остаётся в обходе с `aria-disabled`
 * и `aria-busy`, но нажатие ничего не делает.
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
