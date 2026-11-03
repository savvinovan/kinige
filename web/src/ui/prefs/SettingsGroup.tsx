import type { ReactNode } from "react";
import "./prefs.css";

/**
 * A card-group of settings rows: one fill, thin dividers between rows (as in macOS system settings and Zed).
 * The rows are `ShortcutRow`, `ConfigureRow` and others with a name on the left and an action on the right.
 */
export function SettingsGroup({ children, label }: { children: ReactNode; label?: string }) {
  return (
    <div className="settings-group" role="group" aria-label={label}>
      {children}
    </div>
  );
}
