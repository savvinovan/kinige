import { useId, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { Settings } from "../../api/generated/Settings";
import type { TextSize } from "../../api/generated/TextSize";
import { MirrorStatusLine } from "./MirrorStatusLine";
import { Modal } from "./Modal";
import { PathRow } from "./PathRow";
import { SegmentedChoice } from "./SegmentedChoice";
import { StatusNote } from "./StatusNote";
import "./settings.css";

interface SettingsDialogProps {
  settings: Settings;
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
}

/**
 * Окно «Настройки» (слайс 1): раздел «Копия в Markdown» (папка, предупреждение «копия — открытый текст», состояние) и раздел
 * «Размер текста». Только вид и поведение: данные и действия приходят снаружи, вызовов бэкенда здесь нет.
 * Окно модальное внутри главного (`Modal`): Esc, щелчок по подложке и кнопка закрытия вызывают `onClose`.
 */
export function SettingsDialog({
  settings,
  onClose,
  onChooseFolder,
  onTextSizeChange,
  choosing = false,
  actionError,
  returnFocusRef,
}: SettingsDialogProps) {
  const { t } = useTranslation();
  const mirrorTitleId = useId();
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
