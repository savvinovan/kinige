import { useCopy } from "../message/useCopy";
import { IconButton } from "./IconButton";

/** How long the "copied" tick stays on the button: about one and a half seconds. */
export const COPIED_ICON_MS = 1500;

interface CopyButtonProps {
  /** What goes into the clipboard. */
  text: string;
  /** The button label and tooltip: "Copy path". */
  label: string;
  /** The label while the button shows the tick: "Copied". */
  copiedLabel: string;
  /** The label if the clipboard is unavailable: "Could not copy". */
  failedLabel: string;
  /** The id of the element with the text being copied (`aria-describedby`): the screen reader will read what exactly is copied. */
  describedBy?: string;
}

/**
 * A "Copy" button with an icon (less noise than a text one): a copy icon, after a press a tick for one and a half seconds, on a
 * clipboard failure an error icon. The label and the tooltip (`title`) change together with the icon, so the screen reader hears
 * "Copied". No animation and no toasts. The hit area is the same as for the other icon buttons (`--layout-control-height`).
 */
export function CopyButton({ text, label, copiedLabel, failedLabel, describedBy }: CopyButtonProps) {
  const { state, copy } = useCopy(COPIED_ICON_MS);
  const icon = state === "copied" ? "done" : state === "failed" ? "alert" : "copy";
  const name = state === "copied" ? copiedLabel : state === "failed" ? failedLabel : label;
  return <IconButton icon={icon} label={name} aria-describedby={describedBy} onClick={() => void copy(text)} />;
}
