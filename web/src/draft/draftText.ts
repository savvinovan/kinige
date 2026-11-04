import { useState } from "react";

/**
 * The text in the composer field, kept outside React state: a key press re-renders only the component that reads it
 * (`ComposerPane`), not the window with the feed above it (KINIG-47). Rust never sees this object; it only sits between
 * the typing handlers (`useComposerDraft`) and the field.
 */
export interface DraftText {
  get(): string;
  set(value: string): void;
  /** For `useSyncExternalStore`: returns the unsubscribe function. */
  subscribe(listener: () => void): () => void;
}

export function createDraftText(initial = ""): DraftText {
  let value = initial;
  const listeners = new Set<() => void>();
  return {
    get: () => value,
    set: (next) => {
      if (next === value) return;
      value = next;
      for (const listener of [...listeners]) listener();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

/** One `DraftText` for the lifetime of the component. */
export function useDraftText(initial = ""): DraftText {
  return useState(() => createDraftText(initial))[0];
}
