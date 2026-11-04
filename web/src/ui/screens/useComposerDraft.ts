import { useCallback, useEffect, useRef, useState } from "react";
import { INBOX_ID, type KinigeApi } from "../../api/KinigeApi";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { useDraftText } from "../../draft/draftText";
import { useDraftSaver } from "../../draft/useDraftSaver";

/** What goes into the draft: the text and the entry being replied to (slice 5). */
interface DraftValue {
  body: string;
  replyToId: string | null;
}

/**
 * The text in the main window's composer, the "Reply to entry" strip and a draft per chat: the saved draft is filled in when
 * a chat opens, saving is deferred while typing and immediate when the window hides or loses focus (§5.1). The chat
 * state (`selected`) is kept by `MainWindow`; `switchChat` is called at the same moment as its `setSelected`.
 *
 * The typed text is not React state here: it sits in `text` and only the composer pane subscribes to it, so a key press
 * does not render the window (KINIG-47). Read the current text with `text.get()`.
 */
export function useComposerDraft(api: KinigeApi, selected: string | null) {
  const text = useDraftText();
  const [reply, setReply] = useState<ReplyTo | null>(null);
  // The tiles above the field (slice 7): the chat's unsent attachments. Rust keeps them; they come back with the draft.
  const [tiles, setTiles] = useState<AttachmentView[]>([]);
  // The text and the reply without waiting for a render: needed by handlers that change just one of the two.
  const latest = useRef<{ body: string; reply: ReplyTo | null }>({ body: "", reply: null });
  // Until the person has typed, the draft from Rust is put into the empty field; what was typed before the reply is not overwritten.
  const typed = useRef(false);
  // The open chat without waiting for a render: a `getDraft` reply of the previous chat that arrives after switching does not reach the new chat's field.
  const opened = useRef<string | null>(INBOX_ID);
  const chatId = selected ?? INBOX_ID;

  const save = useCallback((value: DraftValue) => api.saveDraft(chatId, value.body, value.replyToId), [api, chatId]);
  const saver = useDraftSaver<DraftValue>(save);
  const { flush, schedule } = saver;

  // The chat opening number: the draft loading effect runs again even when `selected` did not change (A->B->A before a render).
  const [opening, setOpening] = useState(0);

  const show = useCallback((body: string, replyTo: ReplyTo | null, attachments: AttachmentView[] = []) => {
    latest.current = { body, reply: replyTo };
    text.set(body);
    setReply(replyTo);
    setTiles(attachments);
  }, [text]);

  /** Files were attached (the dialog, a paste, a drop): their tiles join the row. Counts as typing: the saved draft no longer replaces the field. */
  const addTiles = useCallback((added: readonly AttachmentView[]) => {
    if (added.length === 0) return;
    typed.current = true;
    setTiles((was) => [...was, ...added.filter((tile) => !was.some((known) => known.id === tile.id))]);
  }, []);

  /** The x on a tile (after Rust has removed the file). */
  const dropTile = useCallback((attachmentId: string) => {
    setTiles((was) => was.filter((tile) => tile.id !== attachmentId));
  }, []);

  const plan = useCallback(
    (body: string, replyTo: ReplyTo | null) => schedule({ body, replyToId: replyTo?.id ?? null }),
    [schedule],
  );

  /**
   * Another chat: the field is empty until its draft loads (the effect below). The caller saves the previous chat's draft.
   * Selecting the already open chat again does not touch the field: what was typed stays, and so does its scheduled save.
   */
  const switchChat = useCallback(
    (next: string | null) => {
      if (opened.current === next) return;
      typed.current = false;
      opened.current = next;
      show("", null);
      setOpening((count) => count + 1);
    },
    [show],
  );

  /** The person types: the text goes into the field, saving is deferred. */
  const type = useCallback(
    (value: string) => {
      typed.current = true;
      latest.current = { ...latest.current, body: value };
      text.set(value);
      plan(value, latest.current.reply);
    },
    [plan, text],
  );

  /** "Reply" on a card entry (or the strip's cross: `null`): the strip above the composer changes, the draft goes to Rust. */
  const changeReply = useCallback(
    (next: ReplyTo | null) => {
      typed.current = true;
      latest.current = { ...latest.current, reply: next };
      setReply(next);
      plan(latest.current.body, next);
    },
    [plan],
  );

  /** The reply strip was refreshed (after a send error Rust confirmed the entry is gone): the draft is not saved again. */
  const refreshReply = useCallback((next: ReplyTo | null) => {
    latest.current = { ...latest.current, reply: next };
    setReply(next);
  }, []);

  /** The entry was sent: the field and the reply strip are cleared (Rust has already cleared the draft). */
  const clear = useCallback(() => show("", null), [show]);

  // The open chat's draft: until the person has typed, it is put into the empty field together with the reply strip.
  useEffect(() => {
    if (selected === null) return;
    let live = true;
    // React cleans up the previous chat's effect after a render, while `switchChat` empties the field earlier: `live` alone is not enough.
    api
      .getDraft(selected)
      .then((saved) => {
        if (live && opened.current === selected && !typed.current) show(saved.body, saved.replyTo, saved.attachments);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
    // `opening` is in the dependencies: every chat opening restarts the loading, even if `selected` is the same.
  }, [api, selected, opening, show]);

  // The window hides or loses focus: the draft goes to Rust at once, without waiting for a pause in typing.
  useEffect(() => {
    const onHide = () => {
      if (document.visibilityState === "hidden") void flush();
    };
    const onBlur = () => void flush();
    document.addEventListener("visibilitychange", onHide);
    window.addEventListener("blur", onBlur);
    return () => {
      document.removeEventListener("visibilitychange", onHide);
      window.removeEventListener("blur", onBlur);
    };
  }, [flush]);

  return { text, reply, tiles, addTiles, dropTile, changeReply, refreshReply, clear, plan, saver, switchChat, type };
}
