import { useCallback, useEffect, useMemo, useRef } from "react";

/** How long after the last keypress the draft goes to Rust (§5.1: ~300 ms). */
export const DRAFT_DELAY_MS = 300;

export interface DraftSaver<T = string> {
  /** Schedule a draft save: for the same `save` the last call wins, the value of another `save` (another chat) goes to the queue. */
  schedule(body: T): void;
  /** Save now what is scheduled (on hiding the window, loss of focus, closing). Errors are swallowed silently: a draft is a convenience, not data. */
  flush(): Promise<void>;
  /** Cancel what is scheduled: after sending, Rust has already cleared the draft, and a late write would bring it back. */
  cancel(): void;
  /** Wait for the saves that are already sent to Rust (not the scheduled ones): after that, sending an entry will not overtake them. */
  settle(): Promise<void>;
}

/**
 * A delayed draft: while the person types, no request goes to Rust; `DRAFT_DELAY_MS` after the last keypress - one
 * save. On unmount, the scheduled save is performed.
 *
 * Saves go strictly one at a time, in call order: the next starts after the reply to the previous one (Rust commands
 * reach the writer in arbitrary order, and otherwise old text could overwrite new text). `settle()` waits for the tail of
 * this queue - sending an entry uses it, so that a late save does not bring the sent text back as a draft.
 *
 * The scheduled text keeps the `save` function it was scheduled with (in `MainWindow` it is bound to the chat).
 * `flush` takes it from there, not from its own closure: after a chat change React flushes the effects of the previous
 * render with a delay, and the "old" `flush` managed to save the text typed in the new chat into the previous chat's draft.
 */
export function useDraftSaver<T = string>(save: (body: T) => Promise<void>, delayMs = DRAFT_DELAY_MS): DraftSaver<T> {
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pending = useRef<{ body: T; save: (body: T) => Promise<void> } | null>(null);
  const queue = useRef<Promise<void>>(Promise.resolve());

  const enqueue = useCallback((item: { body: T; save: (body: T) => Promise<void> }) => {
    queue.current = queue.current.then(() => item.save(item.body)).catch(() => undefined);
  }, []);

  const flush = useCallback((): Promise<void> => {
    clearTimeout(timer.current);
    timer.current = undefined;
    const item = pending.current;
    pending.current = null;
    if (item !== null) enqueue(item);
    return queue.current;
  }, [enqueue]);

  const schedule = useCallback(
    (body: T) => {
      // The scheduled text of another chat (typed before the switch that is not rendered yet) is not overwritten: it goes to the queue.
      if (pending.current !== null && pending.current.save !== save) enqueue(pending.current);
      pending.current = { body, save };
      clearTimeout(timer.current);
      timer.current = setTimeout(() => void flush(), delayMs);
    },
    [enqueue, flush, save, delayMs],
  );

  const cancel = useCallback(() => {
    clearTimeout(timer.current);
    timer.current = undefined;
    pending.current = null;
  }, []);

  const settle = useCallback(() => queue.current, []);

  useEffect(
    () => () => {
      void flush();
    },
    [flush],
  );

  // The same object while its functions are the same: callers (`select`, the feed) keep it in dependency lists.
  return useMemo(() => ({ schedule, flush, cancel, settle }), [schedule, flush, cancel, settle]);
}
