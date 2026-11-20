import { createContext, useContext, type ReactNode } from "react";

/** What every entry's reaction buttons need from the settings: the three quick emoji and the recent ones for the picker. */
export interface ReactionChoices {
  /** The three emoji of the hover bar and the popover: the recent reactions, then the fixed ❤️ 😂 😢 without repeats. */
  quick: readonly string[];
  /** The reactions put on this device, the freshest first (the picker's "Recent" section in reaction mode). */
  recent: readonly string[];
}

const NONE: ReactionChoices = { quick: [], recent: [] };

const ReactionChoicesContext = createContext<ReactionChoices>(NONE);

export function ReactionChoicesProvider({ value, children }: { value: ReactionChoices; children: ReactNode }) {
  return <ReactionChoicesContext.Provider value={value}>{children}</ReactionChoicesContext.Provider>;
}

export function useReactionChoices(): ReactionChoices {
  return useContext(ReactionChoicesContext);
}
