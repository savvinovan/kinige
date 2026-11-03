import { useCallback, useEffect, useState } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { TrashItem } from "../../api/generated/TrashItem";

interface UseTrashOptions {
  api: KinigeApi;
  /** The trash is open instead of a chat (`selected === null`): the list is read on opening. */
  open: boolean;
  /** Chat and trash counters after an entry changes. */
  refreshChats: () => Promise<void>;
  /** Slice 6: a restored entry is a live entry again, so the forest has to be read again. */
  onRestored?: () => void;
}

/** Trash (slice 3): the list of entries kept for 30 days, "Restore" and "Delete forever" (confirmation is in `TrashView`). */
export function useTrash({ api, open, refreshChats, onRestored }: UseTrashOptions) {
  const [items, setItems] = useState<TrashItem[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<ErrorCode | null>(null);

  useEffect(() => {
    if (!open) return;
    let live = true;
    api
      .listTrash()
      .then((list) => live && setItems(list))
      .catch((reason: unknown) => live && setError(errorCode(reason)));
    return () => {
      live = false;
    };
  }, [api, open]);

  const remove = useCallback(
    (request: Promise<void>, id: string) => {
      setBusyId(id);
      request
        .then(() => {
          setItems((list) => list.filter((item) => item.message.id !== id));
          return refreshChats();
        })
        .catch((reason: unknown) => setError(errorCode(reason)))
        .finally(() => setBusyId(null));
    },
    [refreshChats],
  );

  const restore = useCallback(
    (id: string) =>
      remove(
        api.restoreMessage(id).then(() => onRestored?.()),
        id,
      ),
    [api, remove, onRestored],
  );
  const purge = useCallback((id: string) => remove(api.purgeMessage(id), id), [api, remove]);
  const clearError = useCallback(() => setError(null), []);

  return { items, busyId, error, restore, purge, clearError };
}
