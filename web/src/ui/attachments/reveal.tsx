import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import type { AttachmentView } from "../../api/generated/AttachmentView";

/** Which hidden photos are shown right now. It lives in memory, per chat view: nothing about a reveal is stored anywhere (KINIG-136). */
export interface Reveal {
  isRevealed: (attachmentId: string) => boolean;
  reveal: (attachmentId: string) => void;
  hide: (attachmentId: string) => void;
}

/** Without a provider nothing is revealed and nothing can be: a hidden photo fails closed. */
const ALL_COVERED: Reveal = { isRevealed: () => false, reveal: () => undefined, hide: () => undefined };

export const RevealContext = createContext<Reveal>(ALL_COVERED);

export function useReveal(): Reveal {
  return useContext(RevealContext);
}

/** Whether the photo is covered right now: it is a spoiler and its click has not happened in this view. */
export function useCovered(image: AttachmentView): boolean {
  const { isRevealed } = useReveal();
  return image.spoiler && !isRevealed(image.id);
}

interface RevealProviderProps {
  /** The chat view (a chat id, or the trash): when it changes every photo is covered again. */
  viewKey: string | null;
  children: ReactNode;
}

/**
 * Holds the revealed ids for one chat view. Leaving the chat or the trash, and the window going away (closed to the menu bar, hidden), covers
 * everything again; so does a restart, because nothing is saved. The viewer, the details dialog and the feed share this one state.
 */
export function RevealProvider({ viewKey, children }: RevealProviderProps) {
  const [revealed, setRevealed] = useState<{ viewKey: string | null; ids: ReadonlySet<string> }>({ viewKey, ids: new Set() });
  // A set that belongs to another view counts as empty: the cover is back in the same render as the new view, not one render later.
  const ids = revealed.viewKey === viewKey ? revealed.ids : EMPTY;

  const reveal = useCallback(
    (attachmentId: string) =>
      setRevealed((was) => ({ viewKey, ids: new Set([...(was.viewKey === viewKey ? was.ids : EMPTY), attachmentId]) })),
    [viewKey],
  );
  const hide = useCallback(
    (attachmentId: string) =>
      setRevealed((was) => ({ viewKey, ids: new Set([...(was.viewKey === viewKey ? was.ids : EMPTY)].filter((id) => id !== attachmentId)) })),
    [viewKey],
  );

  useEffect(() => {
    const onVisibility = () => {
      if (document.visibilityState === "hidden") setRevealed({ viewKey, ids: EMPTY });
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, [viewKey]);

  const value = useMemo<Reveal>(() => ({ isRevealed: (id) => ids.has(id), reveal, hide }), [ids, reveal, hide]);
  return <RevealContext.Provider value={value}>{children}</RevealContext.Provider>;
}

const EMPTY: ReadonlySet<string> = new Set();
