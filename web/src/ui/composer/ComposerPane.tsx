import { type ComponentProps, useSyncExternalStore } from "react";
import { type DraftText, useDraftText } from "../../draft/draftText";
import { Composer } from "./Composer";

type ComposerProps = ComponentProps<typeof Composer>;

interface ComposerPaneProps extends Omit<ComposerProps, "value" | "onChange"> {
  /** The text lives here, not in the parent's state. Without it the pane keeps its own (stories). */
  text?: DraftText;
  /** The text the pane starts with when it keeps the text itself. */
  initialText?: string;
  /**
   * The person typed. With a handler, it puts the text into `text` itself (it may refuse, for instance while an entry is being
   * sent); without one the pane stores it.
   */
  onChange?: (value: string) => void;
}

/**
 * The composer with the text owned below the window (KINIG-47): typing re-renders this component and nothing above it,
 * so the feed is not redrawn on every key. Everything else goes to `Composer` unchanged.
 */
export function ComposerPane({ text, initialText, onChange, ...composer }: ComposerPaneProps) {
  const own = useDraftText(initialText);
  const store = text ?? own;
  const value = useSyncExternalStore(store.subscribe, store.get);
  return <Composer {...composer} value={value} onChange={onChange ?? store.set} />;
}
