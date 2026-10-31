import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { TextSize } from "../../../api/generated/TextSize";
import { SegmentedChoice } from "../../settings/SegmentedChoice";
import { ChangedByFile } from "../ChangedByFile";
import "../../settings/settings.css";

interface AppearancePageProps {
  textSize: TextSize;
  onTextSizeChange: (size: TextSize) => void;
  /** Когда раздел менялся через файл (UTC, мс). */
  changedAt: number | null;
  now?: Date;
}

/** «Оформление»: размер текста. */
export function AppearancePage({ textSize, onTextSizeChange, changedAt, now }: AppearancePageProps) {
  const { t } = useTranslation();
  const titleId = useId();
  return (
    <section className="prefs-page" aria-labelledby={titleId}>
      <h2 className="prefs-page__title" id={titleId}>
        {t("prefs.section.appearance")}
      </h2>
      <div className="prefs-block">
        <h3 className="settings__heading" id={`${titleId}-size`}>
          {t("settings.textSize.title")}
        </h3>
        <p className="settings__description">{t("prefs.appearance.description")}</p>
        <SegmentedChoice<TextSize>
          labelledBy={`${titleId}-size`}
          value={textSize}
          onChange={onTextSizeChange}
          options={[
            { value: "normal", label: t("settings.textSize.normal") },
            { value: "large", label: t("settings.textSize.large") },
          ]}
        />
        <ChangedByFile at={changedAt} now={now} />
      </div>
    </section>
  );
}
