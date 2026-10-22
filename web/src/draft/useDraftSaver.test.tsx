import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DRAFT_DELAY_MS, useDraftSaver } from "./useDraftSaver";

afterEach(() => vi.useRealTimers());

describe("useDraftSaver", () => {
  it("одно сохранение через 300 мс после последнего вызова", async () => {
    vi.useFakeTimers();
    const save = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useDraftSaver(save));
    result.current.schedule("а");
    await vi.advanceTimersByTimeAsync(DRAFT_DELAY_MS - 1);
    result.current.schedule("аб");
    await vi.advanceTimersByTimeAsync(DRAFT_DELAY_MS - 1);
    expect(save).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(2);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith("аб");
  });

  it("flush сохраняет запланированное немедленно и только один раз", async () => {
    vi.useFakeTimers();
    const save = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useDraftSaver(save));
    result.current.schedule("текст");
    await result.current.flush();
    await result.current.flush();
    await vi.advanceTimersByTimeAsync(1000);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("cancel отменяет запланированное", async () => {
    vi.useFakeTimers();
    const save = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useDraftSaver(save));
    result.current.schedule("не нужно");
    result.current.cancel();
    await result.current.flush();
    await vi.advanceTimersByTimeAsync(1000);
    expect(save).not.toHaveBeenCalled();
  });

  it("при размонтировании запланированное сохраняется", () => {
    vi.useFakeTimers();
    const save = vi.fn().mockResolvedValue(undefined);
    const { result, unmount } = renderHook(() => useDraftSaver(save));
    result.current.schedule("ушёл");
    unmount();
    expect(save).toHaveBeenCalledWith("ушёл");
  });

  it("ошибка сохранения молча проглатывается: черновик — удобство", async () => {
    const save = vi.fn().mockRejectedValue(new Error("нет связи"));
    const { result } = renderHook(() => useDraftSaver(save));
    result.current.schedule("x");
    await expect(result.current.flush()).resolves.toBeUndefined();
  });
});
