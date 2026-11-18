import { useTranslation } from "react-i18next";
import { IconButton } from "../controls/IconButton";

interface JumpToLatestProps {
  onJump: () => void;
}

/**
 * "Jump to latest" (KINIG-145): a small round button at the bottom right of the feed, as in Telegram Desktop. It overlays the feed and
 * takes no room in it (`.feed-pane`), has no text, and its name is the tooltip and the screen reader label.
 */
export function JumpToLatest({ onJump }: JumpToLatestProps) {
  const { t } = useTranslation();
  return <IconButton icon="chevron" label={t("feed.jumpToLatest")} className="jump-latest" onClick={onJump} />;
}
