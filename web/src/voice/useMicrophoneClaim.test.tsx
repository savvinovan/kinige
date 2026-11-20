import { describe, expect, it, vi } from "vitest";
import { act, renderHook } from "../test/render";
import { useMicrophoneClaim } from "./useMicrophoneClaim";

// The window's side of the claim on its own (KINIG-153): what the recorder tests cannot reach, a window that goes away while Rust is still
// answering the request.

function deferred() {
  let resolve: (granted: boolean) => void = () => undefined;
  const promise = new Promise<boolean>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("a window unmounted while the claim's answer is pending", () => {
  it("lets the claim go the moment it is granted, and reports no microphone", async () => {
    const answer = deferred();
    const holdMicrophone = vi.fn((held: boolean) => (held ? answer.promise : Promise.resolve(true)));
    const hook = renderHook(() => useMicrophoneClaim({ holdMicrophone }));
    let taken: Promise<boolean> = Promise.resolve(false);
    act(() => {
      taken = hook.result.current.take();
    });
    hook.unmount();
    // Nothing was granted yet, so nothing is given back yet.
    expect(holdMicrophone).toHaveBeenCalledTimes(1);
    await act(async () => {
      answer.resolve(true);
      await Promise.resolve();
    });
    await expect(taken).resolves.toBe(false);
    expect(holdMicrophone).toHaveBeenLastCalledWith(false);
    expect(holdMicrophone).toHaveBeenCalledTimes(2);
  });

  it("gives back nothing when the answer was a refusal", async () => {
    const answer = deferred();
    const holdMicrophone = vi.fn((held: boolean) => (held ? answer.promise : Promise.resolve(true)));
    const hook = renderHook(() => useMicrophoneClaim({ holdMicrophone }));
    act(() => {
      void hook.result.current.take();
    });
    hook.unmount();
    await act(async () => {
      answer.resolve(false);
      await Promise.resolve();
    });
    expect(holdMicrophone).toHaveBeenCalledTimes(1);
  });
});

describe("a window that holds the claim", () => {
  it("lets it go once when it unmounts, and not at all when it never held it", async () => {
    const holdMicrophone = vi.fn(() => Promise.resolve(true));
    const idle = renderHook(() => useMicrophoneClaim({ holdMicrophone }));
    idle.unmount();
    expect(holdMicrophone).not.toHaveBeenCalled();
    const holder = renderHook(() => useMicrophoneClaim({ holdMicrophone }));
    await act(async () => {
      await holder.result.current.take();
    });
    holder.unmount();
    expect(holdMicrophone.mock.calls).toEqual([[true], [false]]);
  });
});
