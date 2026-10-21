import type { ReactNode } from "react";
import { Icon, type IconName } from "../icons/Icon";
import "./settings.css";

interface StatusNoteProps {
  /** error — ошибка цветом ошибки; quiet — спокойное пояснение или состояние. */
  tone: "error" | "quiet";
  /** Значок слева: состояние читается не только по цвету. */
  icon: IconName;
  /** alert — ошибка, диктор объявляет её сразу; status — вежливое объявление; без роли — постоянная подсказка. */
  role?: "alert" | "status";
  children: ReactNode;
}

/** Строка состояния под полем: значок и текст. Ошибка — `role="alert"` и цвет ошибки из токенов, тихая — вторичный цвет текста. */
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
