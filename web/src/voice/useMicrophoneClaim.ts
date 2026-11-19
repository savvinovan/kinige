import { useCallback, useEffect, useMemo, useRef } from "react";
import type { KinigeApi } from "../api/KinigeApi";

export interface MicrophoneClaim {
  /** Asks for the shared microphone before the device is opened. False: the other window is recording (or the answer failed); nothing is held. */
  take: () => Promise<boolean>;
  /** Lets it go; does nothing when this window holds nothing. Called on every path that ends the recorder's use of the device. */
  release: () => void;
}

/**
 * The window's side of the microphone claim (KINIG-153). Two windows record, the main one and the capture window, each with its own recorder
 * and one device between them; Rust keeps who holds it (`MicrophoneClaim`), so a second recording is refused instead of started, and the capture
 * window does not hide on a loss of focus while it holds it. Closing the window while it holds the claim lets it go.
 */
export function useMicrophoneClaim(api: Pick<KinigeApi, "holdMicrophone">): MicrophoneClaim {
  const held = useRef(false);
  const alive = useRef(true);
  const latest = useRef(api);
  useEffect(() => {
    latest.current = api;
  });

  const release = useCallback(() => {
    if (!held.current) return;
    held.current = false;
    void latest.current.holdMicrophone(false).catch(() => undefined);
  }, []);

  const take = useCallback(async () => {
    let granted = false;
    try {
      granted = await latest.current.holdMicrophone(true);
    } catch {
      // No answer is no microphone: the recorder shows "unavailable" rather than opening a device nobody has claimed.
    }
    held.current = granted;
    // The window went away while Rust was answering: the claim must not outlive it.
    if (!alive.current) release();
    return granted && alive.current;
  }, [release]);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      release();
    };
  }, [release]);

  return useMemo(() => ({ take, release }), [take, release]);
}
