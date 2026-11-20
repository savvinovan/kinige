import { useTranslation } from "react-i18next";
import { IconButton } from "../controls/IconButton";
import "./forest.css";

export interface ForestToggleProps {
  /** The forest is hidden: the button is not pressed. */
  hidden: boolean;
  onToggle: () => void;
}

/**
 * The on/off switch of the forest pane (KINIG-134): a quiet icon button at the right end of the main window's top bar. Its place
 * and its name stay the same in both states, `aria-pressed` carries the state; only the tooltip names the action.
 */
export function ForestToggle({ hidden, onToggle }: ForestToggleProps) {
  const { t } = useTranslation();
  return (
    <IconButton
      small
      icon="forest"
      className="forest-toggle"
      label={t("forest.toggle")}
      title={t(hidden ? "forest.show" : "forest.hide")}
      aria-pressed={!hidden}
      onClick={onToggle}
    />
  );
}
