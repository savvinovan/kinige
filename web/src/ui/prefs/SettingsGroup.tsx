import type { ReactNode } from "react";
import "./prefs.css";

/**
 * Карточка-группа строк настроек: одна заливка, тонкие разделители между строками (как в системных настройках macOS и Zed).
 * Строки — `ShortcutRow`, `ConfigureRow` и другие, у которых слева название, справа действие.
 */
export function SettingsGroup({ children, label }: { children: ReactNode; label?: string }) {
  return (
    <div className="settings-group" role="group" aria-label={label}>
      {children}
    </div>
  );
}
