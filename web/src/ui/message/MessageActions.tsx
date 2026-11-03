import { useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../controls/IconButton";
import type { IconName } from "../icons/Icon";
import { positionBelow, type MenuPosition } from "../menu/ContextMenu";
import type { CopyState } from "./useCopy";

const COPY_KEY = { idle: "message.copy", copied: "message.copied", failed: "message.copyFailed" } as const;

interface MessageActionsProps {
  copyState: CopyState;
  onCopy: () => void;
  isTask: boolean;
  favorite: boolean;
  /** Read-only entry (group newer than the supported version): everything except "Copy" is disabled. */
  readOnly: boolean;
  /** Handlers without an argument: Message decides "turn on / off" from the current state. No handler - the button is only drawn. */
  onEdit?: () => void;
  onToggleTask?: () => void;
  onToggleFavorite?: () => void;
  /** "More": open the context menu at the button. */
  onMore: (position: MenuPosition) => void;
  menuOpen: boolean;
}

/**
 * Action row to the right of the bubble, level with its bottom line (next to the time): copy, edit, task,
 * favorite, "more". Visible on hover over the message, while its menu is open and when focus is inside it (styles in message.css);
 * in a narrow window with no room on the right, Message puts it over the top right corner of the bubble itself. It is a toolbar
 * with a single Tab stop: arrows, Home and End move between the buttons.
 *
 * States: for a task the "task" icon is accent, for a favorite the star is filled; the screen reader label changes ("Add to favorites" <->
 * "Remove from favorites"). A button without a handler is only drawn (`aria-disabled`, "Coming later"); for a read-only
 * entry it is disabled with an explanation of why.
 */
export function MessageActions({
  copyState,
  onCopy,
  isTask,
  favorite,
  readOnly,
  onEdit,
  onToggleTask,
  onToggleFavorite,
  onMore,
  menuOpen,
}: MessageActionsProps) {
  const { t } = useTranslation();
  const [active, setActive] = useState(0);
  const bar = useRef<HTMLDivElement>(null);

  const buttons = () => [...(bar.current?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
  const move = (to: number) => {
    const list = buttons();
    const index = (to + list.length) % list.length;
    setActive(index);
    list[index]?.focus();
  };
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const current = buttons().indexOf(document.activeElement as HTMLButtonElement);
    const keys: Record<string, number> = { ArrowRight: current + 1, ArrowLeft: current - 1, Home: 0, End: -1 };
    if (!(event.key in keys)) return;
    event.preventDefault();
    move(keys[event.key] ?? 0);
  };

  // Entry action button: no handler - "Coming later", read-only entry - disabled with an explanation.
  const action = (index: number, icon: IconName, label: string, handler: (() => void) | undefined, on = false) => (
    <IconButton
      small
      icon={icon}
      label={label}
      className={on ? `message-actions__on${icon === "favorite" ? " message-actions__filled" : ""}` : undefined}
      unavailable={!handler && !readOnly}
      {...(readOnly ? { "aria-disabled": true, title: `${label}. ${t("error.read_only")}` } : {})}
      tabIndex={active === index ? 0 : -1}
      onFocus={() => setActive(index)}
      onClick={() => {
        if (!readOnly) handler?.();
      }}
    />
  );

  return (
    // The outer layer is the space to the right of the bubble with a transparent gap (the mouse moves from the bubble to the row without losing hover);
    // the inner one is the row itself. Message compares the outer layer's width with the free space of the column.
    <div className="message-actions">
      <div className="message-actions__bar" role="toolbar" aria-label={t("message.actions")} ref={bar} onKeyDown={onKeyDown}>
        <IconButton
          small
          icon={copyState === "copied" ? "done" : "copy"}
          label={t(COPY_KEY[copyState])}
          tabIndex={active === 0 ? 0 : -1}
          onFocus={() => setActive(0)}
          onClick={onCopy}
        />
        {action(1, "edit", t("message.edit"), onEdit)}
        {action(2, "task", isTask ? t("message.untask") : t("message.task"), onToggleTask, isTask)}
        {action(3, "favorite", favorite ? t("message.unfavorite") : t("message.favorite"), onToggleFavorite, favorite)}
        <IconButton
          small
          icon="more"
          label={t("message.more")}
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          tabIndex={active === 4 ? 0 : -1}
          onFocus={() => setActive(4)}
          onClick={(event) => onMore(positionBelow(event.currentTarget))}
        />
        <span className="visually-hidden" role="status">
          {copyState === "idle" ? "" : t(COPY_KEY[copyState])}
        </span>
      </div>
    </div>
  );
}
