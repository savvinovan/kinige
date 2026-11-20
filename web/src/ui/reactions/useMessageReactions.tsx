import { useState, type ReactNode, type RefObject } from "react";
import type { ReactionView } from "../../api/generated/ReactionView";
import { useLanguage } from "../../i18n/I18n";
import type { ReactionBar } from "../message/MessageActions";
import type { MenuPosition } from "../menu/ContextMenu";
import { ReactionPicker, type PickerAnchor } from "./ReactionPicker";
import { ReactionRow } from "./ReactionRow";
import { useReactionChoices } from "./ReactionsContext";

interface Options {
  reactions: readonly ReactionView[];
  keysHad: readonly string[];
  /** Put (`on`) or remove a reaction. Without it nothing can be changed: the chips are labels and the bar has no reaction buttons. */
  onReact?: (emoji: string, on: boolean) => void;
  readOnly: boolean;
  /** The bar is on screen (hover, focus inside, a menu open): the order of the quick emoji is kept while it is. */
  barShown: boolean;
  wrap: RefObject<HTMLElement | null>;
  plate: RefObject<HTMLElement | null>;
}

interface Picker {
  anchor: PickerAnchor;
  start: "quick" | "full";
  from: "bar" | "menu";
}

/** The quick emoji as they were when the bar appeared: a click never moves the next emoji under the pointer. */
function useFrozen<T>(value: T, frozen: boolean): T {
  const [kept, setKept] = useState(value);
  // Adjusting state while rendering, as React advises: while the bar is not held the kept copy follows the value.
  if (!frozen && kept !== value) setKept(value);
  return frozen ? kept : value;
}

/**
 * Everything a bubble needs for reactions (slice 5a): the row of chips, the reaction buttons of the hover bar, and the picker panel that the
 * bar's "Add reaction" button and the menu item "Add reaction…" open. A panel returns focus to the control that opened it, found when it closes
 * (WebKit does not focus a button on click, so the element focused at open time is not a safe thing to remember).
 */
export function useMessageReactions({ reactions, keysHad, onReact, readOnly, barShown, wrap, plate }: Options) {
  const language = useLanguage();
  const { quick, recent } = useReactionChoices();
  const [picker, setPicker] = useState<Picker | null>(null);
  const shownQuick = useFrozen(quick, barShown || picker !== null);
  const state = { reactions, keysHad };

  const bar: ReactionBar | undefined =
    onReact === undefined
      ? undefined
      : {
          quick: shownQuick,
          state,
          onReact,
          onPick: (anchor) => setPicker({ anchor, start: "full", from: "bar" }),
        };

  const close = (returnFocus: boolean) => {
    const from = picker?.from;
    setPicker(null);
    if (!returnFocus) return;
    const target = from === "bar" ? wrap.current?.querySelector<HTMLElement>(".message-actions__react") : plate.current;
    target?.focus();
  };

  return {
    /** The reaction part of the hover bar; `undefined` when the entry cannot take reactions from here. */
    bar,
    /** Chips under the text: removable when the entry can be changed, plain labels otherwise. */
    row: <ReactionRow reactions={reactions} onRemove={onReact && !readOnly ? (emoji) => onReact(emoji, false) : undefined} />,
    has: reactions.length > 0,
    /** "Add reaction…" of the menu: the quick popover at the menu's position. */
    openFromMenu: onReact && !readOnly ? (at: MenuPosition) => setPicker({ anchor: { left: at.x, top: at.y, right: at.x, bottom: at.y }, start: "quick", from: "menu" }) : undefined,
    panel: (picker && onReact ? (
      <ReactionPicker
        anchor={picker.anchor}
        start={picker.start}
        quick={quick}
        recent={recent}
        state={state}
        language={language}
        onReact={onReact}
        onClose={close}
      />
    ) : null) as ReactNode,
  };
}
