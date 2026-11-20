import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const listen = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (...args: unknown[]) => listen(...args) }));

const { TauriApi } = await import("./TauriApi");
const { parseProgress } = await import("./tauriStickers");

describe("the sticker, GIF and token commands", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
  });

  it("names and arguments: camelCase, a missing reply is null, no argument where Rust takes none", async () => {
    const api = new TauriApi();
    invoke.mockResolvedValue({ recent: [], packs: [], hasToken: false });
    await api.getStickerLibrary();
    expect(invoke).toHaveBeenLastCalledWith("get_sticker_library", undefined);
    invoke.mockResolvedValue({ title: "T", added: 1, skipped: 0 });
    await api.addStickerPack("https://t.me/addstickers/TestPack");
    expect(invoke).toHaveBeenLastCalledWith("add_sticker_pack", { link: "https://t.me/addstickers/TestPack" });
    invoke.mockResolvedValue(undefined);
    await api.cancelStickerPack();
    expect(invoke).toHaveBeenLastCalledWith("cancel_sticker_pack", undefined);
    await api.removeStickerPack("pack-1");
    expect(invoke).toHaveBeenLastCalledWith("remove_sticker_pack", { packId: "pack-1" });
    invoke.mockResolvedValue({ id: "m" });
    await api.sendSticker("chat-1", "sticker-1");
    expect(invoke).toHaveBeenLastCalledWith("send_sticker", { chatId: "chat-1", stickerId: "sticker-1", replyToId: null });
    await api.sendSticker("chat-1", "sticker-1", "entry-9");
    expect(invoke).toHaveBeenLastCalledWith("send_sticker", { chatId: "chat-1", stickerId: "sticker-1", replyToId: "entry-9" });
    invoke.mockResolvedValue([]);
    await api.listJournalGifs();
    expect(invoke).toHaveBeenLastCalledWith("list_journal_gifs", undefined);
    invoke.mockResolvedValue({ id: "m" });
    await api.sendGifAgain("chat-1", "att-1", null);
    expect(invoke).toHaveBeenLastCalledWith("send_gif_again", { chatId: "chat-1", attachmentId: "att-1", replyToId: null });
  });

  it("the token commands carry the token only in `set_telegram_bot_token`; the state and the removal carry nothing", async () => {
    const api = new TauriApi();
    invoke.mockResolvedValue({ saved: true });
    await api.getTelegramTokenState();
    expect(invoke).toHaveBeenLastCalledWith("get_telegram_token_state", undefined);
    await api.setTelegramBotToken("123456:ABC");
    expect(invoke).toHaveBeenLastCalledWith("set_telegram_bot_token", { token: "123456:ABC" });
    await api.clearTelegramBotToken();
    expect(invoke).toHaveBeenLastCalledWith("clear_telegram_bot_token", undefined);
  });

  it("the new error codes of Rust become KinigeApiError with their own code, not `internal`", async () => {
    const api = new TauriApi();
    for (const code of ["sticker_pack_cancelled", "secret_store_denied", "secret_store_unavailable", "secret_store_unsupported", "sticker_pack_wait", "telegram_token_invalid"]) {
      invoke.mockRejectedValue({ code });
      await expect(api.addStickerPack("X"), code).rejects.toMatchObject({ code });
    }
    invoke.mockRejectedValue({ code: "not_a_code_we_know" });
    await expect(api.addStickerPack("X")).rejects.toMatchObject({ code: "internal" });
    invoke.mockRejectedValue("a string from the ACL");
    await expect(api.addStickerPack("X")).rejects.toMatchObject({ code: "internal" });
  });

  it("the progress event reaches the handler as counts, and a malformed payload is ignored", async () => {
    let deliver: ((event: { payload: unknown }) => void) | undefined;
    listen.mockImplementation((name: string, handler: (event: { payload: unknown }) => void) => {
      expect(name).toBe("sticker-pack-progress");
      deliver = handler;
      return Promise.resolve(() => undefined);
    });
    const seen: unknown[] = [];
    await new TauriApi().onStickerPackProgress((progress) => seen.push(progress));
    deliver?.({ payload: { done: 12, total: 30 } });
    for (const bad of [null, undefined, "12/30", 12, [], {}, { done: 1 }, { done: "1", total: "2" }, { done: -1, total: 5 }, { done: 1.5, total: 5 }, { done: 9, total: 5 }, { done: NaN, total: 5 }, { done: Infinity, total: Infinity }]) {
      deliver?.({ payload: bad });
    }
    expect(seen).toEqual([{ done: 12, total: 30 }]);
  });
});

describe("parseProgress", () => {
  it("takes two whole non-negative counts with done at most total, and nothing else", () => {
    expect(parseProgress({ done: 0, total: 0 })).toEqual({ done: 0, total: 0 });
    expect(parseProgress({ done: 5, total: 5, extra: "ignored" })).toEqual({ done: 5, total: 5 });
    expect(parseProgress({ done: 6, total: 5 })).toBeNull();
    expect(parseProgress({ done: Number.MAX_SAFE_INTEGER + 1, total: Number.MAX_SAFE_INTEGER + 1 })).toBeNull();
    expect(parseProgress(Object.create(null))).toBeNull();
  });
});
