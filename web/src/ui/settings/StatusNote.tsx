import type { ReactNode } from "react";
import { Icon, type IconName } from "../icons/Icon";
import "./settings.css";

interface StatusNoteProps {
  /** error is an error in the error color; quiet is a calm explanation or state. */
  tone: "error" | "quiet";
  /** The icon on the left: the state is conveyed not only by color. */
  icon: IconName;
  /** alert is an error, the screen reader announces it at once; status is a polite announcement; no role is a permanent hint. */
  role?: "alert" | "status";
  children: ReactNode;
}

/** A status line under a field: an icon and text. An error is `role="alert"` with the error color from tokens, a quiet one uses the secondary text color. */
export function StatusNote({ tone, icon, role, children }: StatusNoteProps) {
  return (
    <p className={`status-note status-note--${tone}`} role={role}>
      <span className="status-note__icon">
        <Icon name={icon} />
      </span>
      <span className="status-note__text">{children}</span>
    </p>
  );
}
