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

  it("сохранения идут по одному, в порядке вызова; settle ждёт уже отправленное", async () => {
    const events: string[] = [];
    const releases: Array<() => void> = [];
    const save = vi.fn(
      (body: string) =>
        new Promise<void>((resolve) => {
          events.push(`start:${body}`);
          releases.push(() => {
            events.push(`done:${body}`);
            resolve();
          });
        }),
    );
    const { result } = renderHook(() => useDraftSaver(save));
    result.current.schedule("а");
    void result.current.flush();
    result.current.schedule("аб");
    void result.current.flush();
    let settled = false;
    void result.current.settle().then(() => (settled = true));
    await Promise.resolve();
    expect(events).toEqual(["start:а"]);
    releases[0]?.();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(events).toEqual(["start:а", "done:а", "start:аб"]);
    expect(settled).toBe(false);
    releases[1]?.();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(events.at(-1)).toBe("done:аб");
    expect(settled).toBe(true);
  });

  it("при размонтировании запланированное сохраняется", async () => {
    vi.useFakeTimers();
    const save = vi.fn().mockResolvedValue(undefined);
    const { result, unmount } = renderHook(() => useDraftSaver(save));
    result.current.schedule("ушёл");
    unmount();
    await vi.advanceTimersByTimeAsync(0);
    expect(save).toHaveBeenCalledWith("ушёл");
  });

  it("текст уходит в тот `save`, с которым его запланировали, даже если flush вызван из прежней отрисовки", async () => {
    // Смена чата меняет `save`; React сбрасывает эффекты прежней отрисовки (в том числе сохранение при размонтировании) с
    // задержкой, уже после того, как человек начал печатать в новом чате.
    const saveInbox = vi.fn().mockResolvedValue(undefined);
    const saveBooks = vi.fn().mockResolvedValue(undefined);
    const { result, rerender } = renderHook(({ save }) => useDraftSaver(save), { initialProps: { save: saveInbox } });
    const previous = result.current;
    rerender({ save: saveBooks });
    result.current.schedule("про книги");
    await previous.flush();
    expect(saveBooks).toHaveBeenCalledWith("про книги");
    expect(saveInbox).not.toHaveBeenCalled();
  });

  it("запланированный текст прежнего чата не затирается набором в новом: он уходит в очередь", async () => {
    const saveInbox = vi.fn().mockResolvedValue(undefined);
    const saveBooks = vi.fn().mockResolvedValue(undefined);
    const { result, rerender } = renderHook(({ save }) => useDraftSaver(save), { initialProps: { save: saveInbox } });
    result.current.schedule("недописанное во входящих");
    // Чат переключён, но запланированное ещё не сохранено; в новом чате начался набор.
    rerender({ save: saveBooks });
    result.current.schedule("про книги");
    await result.current.flush();
    expect(saveInbox).toHaveBeenCalledWith("недописанное во входящих");
    expect(saveBooks).toHaveBeenCalledWith("про книги");
  });

  it("ошибка сохранения молча проглатывается: черновик — удобство", async () => {
    const save = vi.fn().mockRejectedValue(new Error("нет связи"));
    const { result } = renderHook(() => useDraftSaver(save));
    result.current.schedule("x");
    await expect(result.current.flush()).resolves.toBeUndefined();
  });
});
