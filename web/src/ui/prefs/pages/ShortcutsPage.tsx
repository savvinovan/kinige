import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { ShortcutKind } from "../../../api/generated/ShortcutKind";
import type { ShortcutStatus } from "../../../api/generated/ShortcutStatus";
import { ShortcutRow } from "../../settings/ShortcutRow";
import { ChangedByFile } from "../ChangedByFile";
import { SettingsGroup } from "../SettingsGroup";
import "../../settings/settings.css";

interface ShortcutsPageProps {
  shortcuts: readonly ShortcutStatus[];
  /** Уже переведённый текст ошибки у сочетания. */
  errors?: Partial<Record<ShortcutKind, string>>;
  onChange: (kind: ShortcutKind, accelerator: string) => void;
  onReset: (kind: ShortcutKind) => void;
  /** Запись нового сочетания началась или закончилась: Rust на это время отключает глобальные сочетания. */
  onRecording: (active: boolean) => void;
  changedAt: number | null;
  now?: Date;
}

/** «Сочетания клавиш»: два глобальных сочетания со статусом, сменой и сбросом. */
export function ShortcutsPage({ shortcuts, errors, onChange, onReset, onRecording, changedAt, now }: ShortcutsPageProps) {
  const { t } = useTranslation();
  const titleId = useId();
  return (
    <section className="prefs-page" aria-labelledby={titleId}>
      <h2 className="prefs-page__title" id={titleId}>
        {t("prefs.section.shortcuts")}
      </h2>
      <div className="prefs-block">
        <p className="settings__description">{t("settings.shortcuts.description")}</p>
        <SettingsGroup>
          {shortcuts.map((shortcut) => (
            <ShortcutRow
              key={shortcut.kind}
              status={shortcut}
              error={errors?.[shortcut.kind]}
              onChange={(accelerator) => onChange(shortcut.kind, accelerator)}
              onReset={() => onReset(shortcut.kind)}
              onRecording={onRecording}
            />
          ))}
        </SettingsGroup>
        <ChangedByFile at={changedAt} now={now} />
      </div>
    </section>
  );
}
