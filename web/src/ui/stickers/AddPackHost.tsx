import { useEffect, useRef, useState } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import { AddPackDialog, type AddPackState } from "./AddPackDialog";

interface AddPackHostProps {
  api: KinigeApi;
  /** The dialog is done (Cancel, Done, a cancelled job): the window drops it. */
  onClose: () => void;
}

/**
 * The "Add a sticker pack" dialog on the real `KinigeApi`: whether a token is saved is read when it opens (and again when the window comes back from
 * Settings while the dialog says there is none), the job runs in Rust and its counts arrive as events. Cancelling a job closes the dialog without an
 * error (`sticker_pack_cancelled`); any other error is one line above the buttons, and nothing was added.
 */
export function AddPackHost({ api, onClose }: AddPackHostProps) {
  const [hasToken, setHasToken] = useState<boolean | null>(null);
  const [link, setLink] = useState("");
  const [state, setState] = useState<AddPackState>({ phase: "idle", error: null });
  const closed = useRef(false);

  const readToken = () => {
    api.getStickerLibrary().then(
      (library) => {
        if (!closed.current) setHasToken(library.hasToken);
      },
      // Rust will say `sticker_pack_no_token` itself if there is none.
      () => {
        if (!closed.current) setHasToken((known) => known ?? true);
      },
    );
  };

  useEffect(() => {
    closed.current = false;
    readToken();
    let off: (() => void) | undefined;
    api
      .onStickerPackProgress(({ done, total }) => setState((now) => (now.phase === "running" ? { ...now, done, total } : now)))
      .then((unlisten) => {
        if (closed.current) unlisten();
        else off = unlisten;
      })
      .catch(() => undefined);
    return () => {
      closed.current = true;
      off?.();
    };
    // `readToken` only uses `api`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api]);

  // Back from Settings with a token saved there: the "add your bot token" line gives way to the form.
  useEffect(() => {
    if (hasToken !== false) return;
    window.addEventListener("focus", readToken);
    return () => window.removeEventListener("focus", readToken);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hasToken, api]);

  if (hasToken === null) return null;

  const add = () => {
    setState({ phase: "running", done: null, total: null, cancelling: false });
    api.addStickerPack(link).then(
      (result) => {
        if (!closed.current) setState({ phase: "done", result });
      },
      (reason: unknown) => {
        if (closed.current) return;
        const code = errorCode(reason);
        if (code === "sticker_pack_cancelled") onClose();
        else setState({ phase: "idle", error: code });
      },
    );
  };

  return (
    <AddPackDialog
      state={state}
      hasToken={hasToken}
      link={link}
      onLinkChange={(next) => {
        setLink(next);
        if (state.phase === "idle" && state.error) setState({ phase: "idle", error: null });
      }}
      onAdd={add}
      onCancelJob={() => {
        setState((now) => (now.phase === "running" ? { ...now, cancelling: true } : now));
        api.cancelStickerPack().catch(() => undefined);
      }}
      onClose={onClose}
      onOpenSettings={() => void api.openSettings().catch(() => undefined)}
    />
  );
}
