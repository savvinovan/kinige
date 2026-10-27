import { useCallback, useEffect, useState } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { TrashItem } from "../../api/generated/TrashItem";

interface UseTrashOptions {
  api: KinigeApi;
  /** Корзина открыта вместо чата (`selected === null`): список читается при открытии. */
  open: boolean;
  /** Счётчики чатов и корзины после изменения записи. */
  refreshChats: () => Promise<void>;
}

/** Корзина (слайс 3): список записей на 30 дней, «Восстановить» и «Удалить навсегда» (подтверждение — в `TrashView`). */
export function useTrash({ api, open, refreshChats }: UseTrashOptions) {
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

  const restore = useCallback((id: string) => remove(api.restoreMessage(id), id), [api, remove]);
  const purge = useCallback((id: string) => remove(api.purgeMessage(id), id), [api, remove]);
  const clearError = useCallback(() => setError(null), []);

  return { items, busyId, error, restore, purge, clearError };
}
