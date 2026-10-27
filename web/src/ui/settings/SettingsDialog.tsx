import { useId, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { Settings } from "../../api/generated/Settings";
import type { ShortcutKind } from "../../api/generated/ShortcutKind";
import type { ShortcutStatus } from "../../api/generated/ShortcutStatus";
import type { TextSize } from "../../api/generated/TextSize";
import { BackupRow } from "./BackupRow";
import { MirrorStatusLine } from "./MirrorStatusLine";
import { Modal } from "./Modal";
import { PathRow } from "./PathRow";
import { SegmentedChoice } from "./SegmentedChoice";
import { ShortcutRow } from "./ShortcutRow";
import { StatusNote } from "./StatusNote";
import "./settings.css";

interface SettingsDialogProps {
  settings: Settings;
  /** Сочетания клавиш для раздела «Сочетания клавиш» (в приложении — `settings.shortcuts`), по одному на действие. */
  shortcuts: readonly ShortcutStatus[];
  /** Записано новое сочетание (каноничная строка, см. `accelerator.ts`). */
  onShortcutChange: (kind: ShortcutKind, accelerator: string) => void;
  /** «Сбросить» — вернуть сочетание по умолчанию. */
  onShortcutReset: (kind: ShortcutKind) => void;
  /** Запись нового сочетания началась или закончилась: Rust на это время отключает сами глобальные сочетания. */
  onShortcutRecording: (active: boolean) => void;
  /** Уже переведённый текст ошибки сохранения сочетания; показывается как `role="alert"` под строкой этого сочетания. */
  shortcutErrors?: Partial<Record<ShortcutKind, string>>;
  onClose: () => void;
  /** «Выбрать…»: родитель открывает системный диалог (из Rust) и на время выбора ставит `choosing`. */
  onChooseFolder: () => void;
  onTextSizeChange: (size: TextSize) => void;
  /** Системный диалог выбора папки открыт: «Выбрать…» недоступна. */
  choosing?: boolean;
  /** Уже переведённый текст ошибки выбора папки; показывается как `role="alert"` под строкой папки. */
  actionError?: string;
  /** Куда вернуть фокус при закрытии, если открывший элемент к тому времени исчез со страницы (например, пункт меню). */
  returnFocusRef?: RefObject<HTMLElement | null>;
  /** «Сейчас» для метки дня в строке копии базы («сегодня, 09:12»); по умолчанию часы окна. В тестах и историях фиксируется. */
  now?: Date;
}

/**
 * Окно «Настройки»: раздел «Копия в Markdown» (папка, предупреждение «копия — открытый текст», состояние), раздел
 * «Копия базы» (слайс 3: когда снята последняя копия файла базы, сколько копий, папка, ошибки), раздел
 * «Сочетания клавиш» (слайс 2: два сочетания со статусом, смена и сброс) и раздел «Размер текста».
 * Только вид и поведение: данные и действия приходят снаружи, вызовов бэкенда здесь нет.
 * Окно модальное внутри главного (`Modal`): Esc, щелчок по подложке и кнопка закрытия вызывают `onClose`.
 */
export function SettingsDialog({
  settings,
  shortcuts,
  onShortcutChange,
  onShortcutReset,
  onShortcutRecording,
  shortcutErrors,
  onClose,
  onChooseFolder,
  onTextSizeChange,
  choosing = false,
  actionError,
  returnFocusRef,
  now,
}: SettingsDialogProps) {
  const { t } = useTranslation();
  const mirrorTitleId = useId();
  const backupTitleId = useId();
  const shortcutsTitleId = useId();
  const textSizeTitleId = useId();
  const { mirror } = settings;

  return (
    <Modal title={t("settings.title")} closeLabel={t("settings.close")} onClose={onClose} returnFocusRef={returnFocusRef}>
      <section className="settings__section" aria-labelledby={mirrorTitleId}>
        <h3 className="settings__heading" id={mirrorTitleId}>
          {t("settings.mirror.title")}
        </h3>
        <p className="settings__description">{t("settings.mirror.description")}</p>
        <PathRow path={mirror.folder} isDefault={mirror.isDefault} busy={choosing} onChoose={onChooseFolder} />
        {actionError ? (
          <StatusNote tone="error" icon="alert" role="alert">
            {actionError}
          </StatusNote>
        ) : null}
        <MirrorStatusLine mirror={mirror} />
        <StatusNote tone="quiet" icon="info">
          {t("settings.mirror.warning")}
        </StatusNote>
      </section>
      <section className="settings__section" aria-labelledby={backupTitleId}>
        <h3 className="settings__heading" id={backupTitleId}>
          {t("settings.backup.title")}
        </h3>
        <p className="settings__description">{t("settings.backup.description")}</p>
        <BackupRow backup={settings.backup} now={now} />
      </section>
      <section className="settings__section" aria-labelledby={shortcutsTitleId}>
        <h3 className="settings__heading" id={shortcutsTitleId}>
          {t("settings.shortcuts.title")}
        </h3>
        <p className="settings__description">{t("settings.shortcuts.description")}</p>
        <div className="settings__rows">
          {shortcuts.map((shortcut) => (
            <ShortcutRow
              key={shortcut.kind}
              status={shortcut}
              error={shortcutErrors?.[shortcut.kind]}
              onChange={(accelerator) => onShortcutChange(shortcut.kind, accelerator)}
              onReset={() => onShortcutReset(shortcut.kind)}
              onRecording={onShortcutRecording}
            />
          ))}
        </div>
      </section>
      <section className="settings__section" aria-labelledby={textSizeTitleId}>
        <h3 className="settings__heading" id={textSizeTitleId}>
          {t("settings.textSize.title")}
        </h3>
        <SegmentedChoice<TextSize>
          labelledBy={textSizeTitleId}
          value={settings.textSize}
          onChange={onTextSizeChange}
          options={[
            { value: "normal", label: t("settings.textSize.normal") },
            { value: "large", label: t("settings.textSize.large") },
          ]}
        />
      </section>
    </Modal>
  );
}
