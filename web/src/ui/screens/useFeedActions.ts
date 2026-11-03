import { type Dispatch, type SetStateAction, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { MessageFlag } from "../../api/generated/MessageFlag";
import type { MessageView } from "../../api/generated/MessageView";
import type { FeedMessage } from "../feed/buildFeed";
import type { FeedActions } from "../feed/Feed";

/** What the actions of the feed need from `useFeed`: the API, its own helpers and the setters of its state. */
export interface FeedActionsDeps {
  api: KinigeApi;
  refreshChats: () => Promise<void>;
  /** An entry came back changed from Rust: put it in the list in place of the old one. */
  replace: (view: MessageView) => void;
  fail: (reason: unknown) => void;
  reload: () => Promise<void>;
  onTag: (tag: string) => void;
  /** The journal changed (slice 6: the forest reads it again). */
  mutated: () => void;
  setMessages: Dispatch<SetStateAction<FeedMessage[]>>;
  setEditingId: Dispatch<SetStateAction<string | null>>;
  setEditError: Dispatch<SetStateAction<string | undefined>>;
  setDeletedIds: Dispatch<SetStateAction<ReadonlySet<string>>>;
  setDateFor: Dispatch<SetStateAction<string | null>>;
}

/** The actions on one entry of the feed: edit, flags, delete and restore, task items, move to another chat. */
export function useFeedActions({
  api,
  refreshChats,
  replace,
  fail,
  reload,
  onTag,
  mutated,
  setMessages,
  setEditingId,
  setEditError,
  setDeletedIds,
  setDateFor,
}: FeedActionsDeps): FeedActions {
  const { t } = useTranslation();

  const setFlag = useCallback(
    (id: string, flag: MessageFlag, on: boolean) => {
      api.setMessageFlag(id, flag, on).then(replace).catch(fail);
    },
    [api, replace, fail],
  );

  return useMemo(
    () => ({
      onEdit: (id) => {
        setEditError(undefined);
        setEditingId(id);
      },
      onSaveEdit: (id, text) => {
        api
          .editMessage(id, text)
          .then((view) => {
            replace(view);
            setEditingId(null);
            setEditError(undefined);
          })
          .catch((reason: unknown) => setEditError(t(`error.${errorCode(reason)}`)));
      },
      onCancelEdit: () => {
        setEditingId(null);
        setEditError(undefined);
      },
      onToggleTask: (id, on) => setFlag(id, "task", on),
      onToggleDone: (id, done) => setFlag(id, "task_done", done),
      onToggleFavorite: (id, on) => setFlag(id, "favorite", on),
      onChangeDate: (id) => setDateFor(id),
      onDelete: (id) => {
        api
          .deleteMessage(id)
          .then(() => {
            setDeletedIds((ids) => new Set(ids).add(id));
            setEditingId((editing) => (editing === id ? null : editing));
            mutated();
            return refreshChats();
          })
          .catch(fail);
      },
      onRestore: (id) => {
        api
          .restoreMessage(id)
          .then(() => {
            setDeletedIds((ids) => {
              const next = new Set(ids);
              next.delete(id);
              return next;
            });
            mutated();
            return refreshChats();
          })
          .catch(fail);
      },
      onToggleTaskItem: (id, index, checked) => {
        api
          .toggleTaskItem(id, index, checked)
          .then(replace)
          .catch((reason: unknown) => {
            fail(reason);
            // The entry text changed in the meantime: show the current one.
            if (errorCode(reason) === "task_item_stale") void reload();
          });
      },
      onTagClick: onTag,
      onMoveTo: (id, targetChatId) => {
        api
          .moveMessage(id, targetChatId)
          .then(() => {
            // The entry simply disappears from the current chat (no toast, the feed does not jump); counters are fetched again.
            setMessages((list) => list.filter((m) => m.id !== id));
            setEditingId((editing) => (editing === id ? null : editing));
            return refreshChats();
          })
          .catch(fail);
      },
    }),
    [api, t, replace, setFlag, refreshChats, fail, reload, onTag, mutated, setMessages, setEditingId, setEditError, setDeletedIds, setDateFor],
  );
}
