import type { ReactNode } from "react";
import "./frame.css";

/** The backdrop of component stories: the feed ("paper") with margins so that a component stands on its real background. */
export function Frame({
  children,
  surface = "paper",
  narrow = false,
  bottom = false,
}: {
  children: ReactNode;
  /** The background behind the component: the feed (paper) or the sidebar and window (background). */
  surface?: "paper" | "background";
  /** A column width of about 36 characters: a check for wrapping long text. */
  narrow?: boolean;
  /** The component at the bottom edge of the window without the backdrop margins (composer, toast). */
  bottom?: boolean;
}) {
  return (
    <div className={`story-frame story-frame--${surface}${bottom ? " story-frame--bottom" : ""}`}>
      <div className={narrow ? "story-frame__inner story-frame__inner--narrow" : "story-frame__inner"}>{children}</div>
    </div>
  );
}

/** A caption for a group of states inside a story. */
export function FrameLabel({ children }: { children: ReactNode }) {
  return <p className="story-frame__label">{children}</p>;
}
