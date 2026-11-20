import { useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../controls/IconButton";
import type { IconName } from "../icons/Icon";
import { positionBelow, type MenuPosition } from "../menu/ContextMenu";
import { isLive, pickerRefusal, putRefusal, type ReactionState } from "../reactions/reactionLimits";
import { ReactionButton } from "../reactions/ReactionButton";
import type { PickerAnchor } from "../reactions/ReactionPicker";
import type { CopyState } from "./useCopy";

const COPY_KEY = { idle: "message.copy", copied: "message.copied", failed: "message.copyFailed" } as const;

/** The reaction buttons at the start of the row (slice 5a): three quick emoji and the "Add reaction" button that opens the picker. */
export interface ReactionBar {
  /** The three quick emoji; their order is fixed while the pointer is on the entry (the owner freezes it). */
  quick: readonly string[];
  /** The entry's reactions and the keys it has had: what is pressed, and what a full entry cannot take. */
  state: ReactionState;
  onReact: (emoji: string, on: boolean) => void;
  /** Open the picker at the "Add reaction" button. */
  onPick: (anchor: PickerAnchor) => void;
}

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
  /** Reactions (slice 5a). Without it the row is the five buttons of slice 3. */
  reactions?: ReactionBar;
}

/** How many buttons the reaction part of the row has: three emoji and the picker. */
const REACTION_BUTTONS = 4;

/**
 * Action row to the right of the bubble, level with its bottom line (next to the time): three quick emoji and "Add reaction", a thin divider, then
 * copy, edit, task, favorite, "more". Visible on hover over the message, while its menu is open and when focus is inside it (styles in message.css);
 * in a narrow window with no room on the right, Message puts it over the top right corner of the bubble itself. It is a toolbar
 * with a single Tab stop: arrows, Home and End move between the buttons.
 *
 * States: for a task the "task" icon is accent, for a favorite the star is filled; the screen reader label changes ("Add to favorites" <->
 * "Remove from favorites"). A quick emoji is a toggle (`aria-pressed`); one that would add to a full entry is `aria-disabled` and says why. A button
 * without a handler is only drawn (`aria-disabled`, "Coming later"); for a read-only entry it is disabled with an explanation of why.
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
  reactions,
}: MessageActionsProps) {
  const { t } = useTranslation();
  const [active, setActive] = useState(0);
  const bar = useRef<HTMLDivElement>(null);
  // The index of the first of the old buttons: the reaction ones come before it.
  const first = reactions ? REACTION_BUTTONS : 0;

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
      tabIndex={active === first + index ? 0 : -1}
      onFocus={() => setActive(first + index)}
      onClick={() => {
        if (!readOnly) handler?.();
      }}
    />
  );

  const addRefusal = reactions ? pickerRefusal(reactions.state) : null;
  const addLabel = t("reaction.add");
  const addBlocked = readOnly || addRefusal !== null;
  const addReason = readOnly ? t("error.read_only") : addRefusal === "live" ? t("reaction.limitLive") : "";

  return (
    // The outer layer is the space to the right of the bubble with a transparent gap (the mouse moves from the bubble to the row without losing hover);
    // the inner one is the row itself. Message compares the outer layer's width with the free space of the column.
    <div className="message-actions">
      <div className="message-actions__bar" role="toolbar" aria-label={t("message.actions")} ref={bar} onKeyDown={onKeyDown}>
        {reactions ? (
          <>
            {reactions.quick.map((emoji, index) => (
              <ReactionButton
                key={emoji}
                emoji={emoji}
                pressed={isLive(reactions.state, emoji)}
                refusal={putRefusal(reactions.state, emoji)}
                readOnly={readOnly}
                tabIndex={active === index ? 0 : -1}
                onFocus={() => setActive(index)}
                onSwitch={(on) => reactions.onReact(emoji, on)}
              />
            ))}
            <IconButton
              small
              icon="reactionAdd"
              label={addLabel}
              className="message-actions__react"
              aria-haspopup="dialog"
              aria-disabled={addBlocked || undefined}
              title={addBlocked && addReason !== "" ? `${addLabel}. ${addReason}` : addLabel}
              tabIndex={active === REACTION_BUTTONS - 1 ? 0 : -1}
              onFocus={() => setActive(REACTION_BUTTONS - 1)}
              onClick={(event) => {
                if (!addBlocked) reactions.onPick(event.currentTarget.getBoundingClientRect());
              }}
            />
            <span className="message-actions__divider" aria-hidden="true" />
          </>
        ) : null}
        <IconButton
          small
          icon={copyState === "copied" ? "done" : "copy"}
          label={t(COPY_KEY[copyState])}
          tabIndex={active === first ? 0 : -1}
          onFocus={() => setActive(first)}
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
          tabIndex={active === first + 4 ? 0 : -1}
          onFocus={() => setActive(first + 4)}
          onClick={(event) => onMore(positionBelow(event.currentTarget))}
        />
        <span className="visually-hidden" role="status">
          {copyState === "idle" ? "" : t(COPY_KEY[copyState])}
        </span>
      </div>
    </div>
  );
}
