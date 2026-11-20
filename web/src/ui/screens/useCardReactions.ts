import { useCallback, useMemo } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { Settings } from "../../api/generated/Settings";
import type { ReactionChoices } from "../reactions/ReactionsContext";

interface Options {
  api: KinigeApi;
  settings: Settings;
  /** The "On this day" card is read again. */
  refreshCard: () => Promise<void>;
  /** The quick set and the recent list of the settings are read again. */
  refreshReactions: () => void;
  onError: (code: ErrorCode) => void;
}

/**
 * Reactions outside the feed (slice 5a): what the hover bar and the pickers offer (the three quick emoji and the recent ones, from the settings),
 * and a reaction put from the "On this day" card: the card is read again so the chip shows under the entry, and the quick set follows.
 */
export function useCardReactions({ api, settings, refreshCard, refreshReactions, onError }: Options) {
  const choices: ReactionChoices = useMemo(
    () => ({ quick: settings.quickReactions, recent: settings.recentReactions }),
    [settings.quickReactions, settings.recentReactions],
  );
  const reactOnCard = useCallback(
    (id: string, emoji: string, on: boolean) => {
      api
        .setMessageReaction(id, emoji, on)
        .then(() => {
          void refreshCard();
          if (on) refreshReactions();
        })
        .catch((reason: unknown) => onError(errorCode(reason)));
    },
    [api, refreshCard, refreshReactions, onError],
  );
  return { choices, reactOnCard };
}
