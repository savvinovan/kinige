import { ForestToggle } from "../forest/ForestToggle";

interface ChatTopBarProps {
  /** The open chat's name as the sidebar shows it (`displayName`: Inbox is translated); empty when the chat is not in the list. */
  title: string;
  /** The forest switch at the right end; without it the bar carries the name alone. */
  forestMenu?: { hidden: boolean; onToggle: () => void };
}

/**
 * The thin bar at the top of the feed column (KINIG-134, KINIG-150): the open chat's name at the left end, one line cut with
 * an ellipsis, and the forest switch at the right end. The bar has the same height with any title and with the forest shown
 * or hidden, and the switch never moves. The name is a heading, so the screen reader and the full text (a tooltip when the line is cut) have it whole.
 */
export function ChatTopBar({ title, forestMenu }: ChatTopBarProps) {
  return (
    <div className="main-window__topbar">
      <h1 className="main-window__title" title={title}>
        {title}
      </h1>
      {forestMenu ? <ForestToggle hidden={forestMenu.hidden} onToggle={forestMenu.onToggle} /> : null}
    </div>
  );
}
