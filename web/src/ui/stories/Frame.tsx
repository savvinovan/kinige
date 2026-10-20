import type { ReactNode } from "react";
import "./frame.css";

/** Подложка историй компонентов: лента («бумага») с полями, чтобы компонент стоял на своём настоящем фоне. */
export function Frame({
  children,
  surface = "paper",
  narrow = false,
  bottom = false,
}: {
  children: ReactNode;
  /** Фон за компонентом: лента (paper) или сайдбар и окно (background). */
  surface?: "paper" | "background";
  /** Ширина колонки около 36 знаков: проверка переноса длинного текста. */
  narrow?: boolean;
  /** Компонент у нижнего края окна без полей подложки (composer, тост). */
  bottom?: boolean;
}) {
  return (
    <div className={`story-frame story-frame--${surface}${bottom ? " story-frame--bottom" : ""}`}>
      <div className={narrow ? "story-frame__inner story-frame__inner--narrow" : "story-frame__inner"}>{children}</div>
    </div>
  );
}

/** Подпись к группе состояний внутри истории. */
export function FrameLabel({ children }: { children: ReactNode }) {
  return <p className="story-frame__label">{children}</p>;
}
