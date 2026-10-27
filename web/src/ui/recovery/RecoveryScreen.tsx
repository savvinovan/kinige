import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { BackupInfo } from "../../api/generated/BackupInfo";
import type { Language } from "../../api/generated/Language";
import { Button } from "../controls/Button";
import { formatTime } from "../feed/dates";
import { StatusNote } from "../settings/StatusNote";
import "./recovery.css";

interface RecoveryScreenProps {
  /** startup — база не открылась при старте, список копий и «Восстановить»; running — проверка нашла повреждение в работающем приложении. */
  mode: "startup" | "running";
  /** Копии базы в любом порядке: экран сам ставит новые первыми. */
  backups: readonly BackupInfo[];
  language: Language;
  /** Восстановление или перезапуск идёт: кнопка и выбор недоступны. */
  busy?: boolean;
  /** Уже переведённая ошибка (например «Копия не найдена или не открывается»): `role="alert"`. */
  error?: string;
  /** «Восстановить из копии от …»: имя файла выбранной копии. */
  onRestore: (name: string) => void;
  onRestart: () => void;
}

/** «27 октября 2026, 09:12»: день и время копии в часовом поясе окна (русское « г.» после года отброшено). */
function formatWhen(createdAt: number, language: Language): string {
  const date = new Date(createdAt);
  const parts = new Intl.DateTimeFormat(language, { day: "numeric", month: "long", year: "numeric" }).formatToParts(date);
  const last = parts.at(-1);
  if (last?.type === "literal" && /^\s*г\.?$/.test(last.value)) parts.pop();
  return `${parts.map((part) => part.value).join("")}, ${formatTime(date, language)}`;
}

/**
 * Экран «База повреждена»: занимает окно целиком и не модальный (журнал не открыт). Повреждённый файл не удаляется, а
 * откладывается рядом с базой; запись после последней копии могла не сохраниться.
 *
 * startup: пояснение, список копий (новые первыми, радиогруппа; вид копии — «копия дня» / «перед обновлением»), главная
 * кнопка «Восстановить из копии от …» для выбранной (по умолчанию самая новая). Копий нет — тихое сообщение и подсказка
 * без кнопки. running: пояснение «Kinige перезапустится и предложит копии» и кнопка «Перезапустить», списка нет.
 */
export function RecoveryScreen({
  mode,
  backups,
  language,
  busy = false,
  error,
  onRestore,
  onRestart,
}: RecoveryScreenProps) {
  const { t } = useTranslation();
  const sorted = [...backups].sort((a, b) => b.createdAt - a.createdAt);
  const newest = sorted[0];
  const [picked, setPicked] = useState<string | null>(null);
  // Выбранная копия могла пропасть из списка (обновился список): тогда снова самая новая.
  const selected = sorted.find((backup) => backup.name === picked) ?? newest;

  return (
    <main className="recovery" aria-labelledby="recovery-title">
      <div className="recovery__column">
        <h1 className="recovery__title" id="recovery-title">
          {t("recovery.title")}
        </h1>
        <p className="recovery__text">{mode === "running" ? t("recovery.runningText") : t("recovery.startupText")}</p>
        {/* Папка Markdown-копии: путь зависит от выбранной копии (его хранит она), поэтому строка без пути. */}
        {mode === "startup" && newest !== undefined ? (
          <p className="recovery__text recovery__text--quiet">{t("recovery.mirrorNote")}</p>
        ) : null}

        {mode === "startup" && newest === undefined ? (
          <>
            <p className="recovery__none">{t("recovery.none")}</p>
            <p className="recovery__text recovery__text--quiet">{t("recovery.noneHint")}</p>
          </>
        ) : null}

        {mode === "startup" && selected !== undefined ? (
          <div className="recovery__list" role="radiogroup" aria-label={t("recovery.backups")}>
            {sorted.map((backup) => (
              <label className="recovery__backup" key={backup.name}>
                <input
                  type="radio"
                  className="visually-hidden"
                  name="recovery-backup"
                  value={backup.name}
                  checked={backup.name === selected.name}
                  disabled={busy}
                  autoFocus={backup.name === newest?.name}
                  onChange={() => setPicked(backup.name)}
                />
                <span className="recovery__when">{formatWhen(backup.createdAt, language)}</span>
                <span className="recovery__kind">{t(`recovery.kind.${backup.kind}`)}</span>
              </label>
            ))}
          </div>
        ) : null}

        {error ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {error}
          </StatusNote>
        ) : null}

        {mode === "running" ? (
          <Button
            variant="secondary"
            className="recovery__primary"
            autoFocus
            aria-disabled={busy || undefined}
            aria-busy={busy || undefined}
            onClick={() => {
              if (!busy) onRestart();
            }}
          >
            {t("recovery.restart")}
          </Button>
        ) : selected !== undefined ? (
          <Button
            variant="secondary"
            className="recovery__primary"
            aria-disabled={busy || undefined}
            aria-busy={busy || undefined}
            onClick={() => {
              if (!busy) onRestore(selected.name);
            }}
          >
            {busy ? t("recovery.restoring") : t("recovery.restore", { when: formatWhen(selected.createdAt, language) })}
          </Button>
        ) : null}
      </div>
    </main>
  );
}
