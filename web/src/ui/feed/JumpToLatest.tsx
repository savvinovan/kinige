import { useLayoutEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../controls/IconButton";

interface JumpToLatestProps {
  onJump: () => void;
  /** The button went away while it held the focus (not because it was pressed): the browser would drop the focus to `<body>`. */
  onFocusLost?: () => void;
}

/**
 * "Jump to latest" (KINIG-145): a small round button at the bottom right of the feed, as in Telegram Desktop. It overlays the feed and
 * takes no room in it (`.feed-pane`), has no text, and its name is the tooltip and the screen reader label.
 */
export function JumpToLatest({ onJump, onFocusLost }: JumpToLatestProps) {
  const { t } = useTranslation();
  const node = useRef<HTMLButtonElement | null>(null);
  const lost = useRef(onFocusLost);
  useLayoutEffect(() => {
    lost.current = onFocusLost;
  });
  // The cleanup of a layout effect runs before React takes the button out of the page, so the focus is still on it.
  useLayoutEffect(
    () => () => {
      if (node.current !== null && document.activeElement === node.current) lost.current?.();
    },
    [],
  );
  return (
    <IconButton
      icon="chevron"
      label={t("feed.jumpToLatest")}
      className="jump-latest"
      onClick={onJump}
      onFocus={(event) => {
        node.current = event.currentTarget;
      }}
    />
  );
}
