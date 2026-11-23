import { describe, expect, it } from "vitest";
import { INBOX_ID } from "../KinigeApi";
import { MockApi, mockMessage } from "../MockApi";
import { demoPhoto } from "../../ui/attachments/demo";
import { packName } from "../mockStickerRules";

const GIF = demoPhoto("gif-1", "cat.gif", 480, 270, { mime: "image/gif", byteSize: 1_000_000 });

function setup() {
  const api = new MockApi({ language: "en", now: () => new Date(Date.UTC(2026, 10, 20, 12, 0)) });
  const library = api.stickerLibrary;
  const exported = library.seedPack("From Telegram", 3, "telegram_export");
  const [sticker] = exported.stickers;
  if (!sticker) throw new Error("no sticker");
  return { api, library, exported, sticker };
}

describe("the pack link rule (kinige_domain::stickers::pack_name)", () => {
  it("takes a link, a link without the scheme or a bare name", () => {
    expect(packName("https://t.me/addstickers/TestPack")).toBe("TestPack");
    expect(packName("t.me/addstickers/Test_Pack_2")).toBe("Test_Pack_2");
    expect(packName("  TestPack\n")).toBe("TestPack");
  });

  it("refuses everything else: other hosts and schemes, queries, a second segment, odd characters, 65 characters", () => {
    for (const bad of [
      "",
      " ",
      "http://t.me/addstickers/TestPack",
      "https://t.me/addstickers/",
      "https://t.me/addstickers/Test/Pack",
      "https://t.me/addstickers/Test?x=1",
      "https://t.me/addstickers/Test#x",
      "https://example.com/addstickers/Test",
      "https://t.me.evil.example/addstickers/Test",
      "Test Pack",
      "../x",
      "Test%2e",
      "a".repeat(65),
    ]) {
      expect(packName(bad), bad).toBeNull();
    }
  });
});

describe("MockApi: the sticker library and sending", () => {
  it("reads the library: the export's pack first, recent empty, no token", async () => {
    const { api, library } = setup();
    library.seedPack("Cats", 2);
    const read = await api.getStickerLibrary();
    expect(read.packs.map((pack) => pack.title)).toEqual(["From Telegram", "Cats"]);
    expect(read.recent).toEqual([]);
    expect(read.hasToken).toBe(false);
  });

  it("sends a sticker as its own entry: empty body, one sticker picture; the recent list goes first; the text draft is untouched", async () => {
    const { api, sticker, exported } = setup();
    await api.saveDraft(INBOX_ID, "half a thought");
    const sent = await api.sendSticker(INBOX_ID, sticker.id);
    expect(sent.bodyMd).toBe("");
    expect(sent.attachments).toHaveLength(1);
    expect(sent.attachments[0]).toMatchObject({ kind: "image", sticker: true, stickerEmoji: sticker.emoji, mime: "image/webp" });
    expect((await api.getDraft(INBOX_ID)).body).toBe("half a thought");
    const second = exported.stickers[1];
    if (!second) throw new Error("no second sticker");
    await api.sendSticker(INBOX_ID, second.id);
    await api.sendSticker(INBOX_ID, sticker.id);
    expect((await api.getStickerLibrary()).recent.map((s) => s.id)).toEqual([sticker.id, second.id]);
  });

  it("a pending reply goes with the sticker and is checked like any reply", async () => {
    const { api, sticker } = setup();
    const target = await api.sendMessage(INBOX_ID, "an entry");
    const sent = await api.sendSticker(INBOX_ID, sticker.id, target.id);
    expect(sent.replyTo?.id).toBe(target.id);
    await expect(api.sendSticker(INBOX_ID, sticker.id, "nope")).rejects.toMatchObject({ code: "reply_target_not_found" });
  });

  it("refuses what Rust refuses, in Rust's order: a garbage id is no sticker, then the chat, then the library, then the file", async () => {
    const { api, library, sticker } = setup();
    for (const garbage of ["", "x", "../etc", "5c000000-0000-4000-8000-00000000zzzz", `${sticker.id} `, sticker.id.toUpperCase()]) {
      await expect(api.sendSticker(INBOX_ID, garbage), garbage).rejects.toMatchObject({ code: "sticker_not_found" });
    }
    await expect(api.sendSticker("no-chat", sticker.id)).rejects.toMatchObject({ code: "chat_not_found" });
    await expect(api.sendSticker(INBOX_ID, "5c000000-0000-4000-8000-0000000000ff")).rejects.toMatchObject({ code: "sticker_not_found" });
    library.unreadable.add(sticker.id);
    await expect(api.sendSticker(INBOX_ID, sticker.id)).rejects.toMatchObject({ code: "sticker_unreadable" });
    // Nothing was written by any refusal.
    expect((await api.listMessages(INBOX_ID)).messages).toHaveLength(0);
  });

  it("a sticker entry is never edited, has no details and is never an append target", async () => {
    const { api, sticker } = setup();
    const sent = await api.sendSticker(INBOX_ID, sticker.id);
    await expect(api.editMessage(sent.id, "text")).rejects.toMatchObject({ code: "sticker_entry_read_only" });
    const [attachment] = sent.attachments;
    await expect(api.setAttachmentDetails(attachment?.id ?? "", { spoiler: true })).rejects.toMatchObject({ code: "sticker_entry_read_only" });
    // The quick capture window appends to the last Inbox entry when it is recent: not to a sticker.
    expect((await api.captureState()).appendTarget).toBeNull();
    const outcome = await api.captureAppend("a line");
    expect(outcome.action).toBe("created");
  });

  it("a sticker that was given a body (an edited import) or a cover is not a sticker entry, so it can still be edited and uncovered", async () => {
    const { api } = setup();
    const covered = demoPhoto("c1", "sticker.webp", 512, 512, { sticker: { emoji: "\u{1F63A}" }, spoiler: true });
    const withText = demoPhoto("t1", "sticker.webp", 512, 512, { sticker: { emoji: "\u{1F63A}" } });
    api.addMessageWithAttachments({ id: "m-covered", bodyMd: "" }, [covered]);
    api.addMessageWithAttachments({ id: "m-text", bodyMd: "an old note" }, [withText]);
    await expect(api.setAttachmentDetails("c1", { spoiler: false })).resolves.toMatchObject({ spoiler: false });
    await expect(api.editMessage("m-text", "a new note")).resolves.toMatchObject({ bodyMd: "a new note" });
  });
});

describe("MockApi: packs by link", () => {
  it("without a token nothing starts: sticker_pack_no_token, and the pretend Telegram was not asked", async () => {
    const { api, library } = setup();
    await expect(api.addStickerPack("TestPack")).rejects.toMatchObject({ code: "sticker_pack_no_token" });
    expect(library.requested).toEqual([]);
  });

  it("a bad link is sticker_pack_link_invalid, an unknown name sticker_pack_not_found", async () => {
    const { api } = setup();
    await api.setTelegramBotToken("123456:ABC-def_1");
    await expect(api.addStickerPack("https://example.com/addstickers/X")).rejects.toMatchObject({ code: "sticker_pack_link_invalid" });
    await expect(api.addStickerPack("NoSuchPack")).rejects.toMatchObject({ code: "sticker_pack_not_found" });
  });

  it("adds the pack with progress counts, and the result says how many were added and skipped", async () => {
    const { api } = setup();
    await api.setTelegramBotToken("123456:ABC-def_1");
    const seen: [number, number][] = [];
    await api.onStickerPackProgress(({ done, total }) => seen.push([done, total]));
    const added = await api.addStickerPack("https://t.me/addstickers/TestPack");
    expect(added).toEqual({ title: "Test Pack", added: 27, skipped: 3 });
    expect(seen.at(-1)).toEqual([30, 30]);
    const library = await api.getStickerLibrary();
    expect(library.packs.map((pack) => pack.title)).toEqual(["From Telegram", "Test Pack"]);
    expect(library.packs[1]?.stickers).toHaveLength(27);
  });

  it("the same name again replaces the pack; one job at a time; cancel (twice) adds nothing", async () => {
    const { api, library } = setup();
    await api.setTelegramBotToken("123456:ABC-def_1");
    await api.addStickerPack("TestPack");
    await api.addStickerPack("TestPack");
    expect((await api.getStickerLibrary()).packs.filter((pack) => pack.title === "Test Pack")).toHaveLength(1);

    library.hold = true;
    const running = api.addStickerPack("TestPack");
    await expect(api.addStickerPack("TestPack")).rejects.toMatchObject({ code: "sticker_pack_busy" });
    await api.cancelStickerPack();
    await api.cancelStickerPack();
    await expect(running).rejects.toMatchObject({ code: "sticker_pack_cancelled" });
    // The old pack is untouched by the cancelled replacement.
    expect((await api.getStickerLibrary()).packs.filter((pack) => pack.title === "Test Pack")).toHaveLength(1);
    // Cancel with no job does nothing.
    await expect(api.cancelStickerPack()).resolves.toBeUndefined();
  });

  it("a pack with only animated stickers, or one the pretend Telegram refuses, adds nothing", async () => {
    const { api, library } = setup();
    await api.setTelegramBotToken("123456:ABC-def_1");
    library.remotePacks.set("Moving", { title: "Moving", stickers: 0, skipped: 12 });
    library.remotePacks.set("Big", { title: "Big", stickers: 5, error: "sticker_pack_too_large" });
    await expect(api.addStickerPack("Moving")).rejects.toMatchObject({ code: "sticker_pack_only_animated" });
    await expect(api.addStickerPack("Big")).rejects.toMatchObject({ code: "sticker_pack_too_large" });
    expect((await api.getStickerLibrary()).packs.map((pack) => pack.title)).toEqual(["From Telegram"]);
  });

  it("removes a pack by id; a garbage or unknown id is sticker_not_found; sent stickers stay in the entries", async () => {
    const { api, exported, sticker } = setup();
    const sent = await api.sendSticker(INBOX_ID, sticker.id);
    await api.removeStickerPack(exported.id);
    expect((await api.getStickerLibrary()).packs).toEqual([]);
    expect(api.messageById(sent.id)?.attachments).toHaveLength(1);
    await expect(api.removeStickerPack(exported.id)).rejects.toMatchObject({ code: "sticker_not_found" });
    await expect(api.removeStickerPack("../x")).rejects.toMatchObject({ code: "sticker_not_found" });
  });
});

describe("MockApi: GIFs of the journal", () => {
  it("lists live uncovered GIFs, newest first, one per content; sending again makes a new entry without the source's details", async () => {
    const { api } = setup();
    const older = { ...GIF, id: "gif-old", filename: "old.gif" };
    api.addMessageWithAttachments({ id: "m1", bodyMd: "", createdAt: Date.UTC(2026, 10, 1) }, [older]);
    api.addMessageWithAttachments({ id: "m2", bodyMd: "", createdAt: Date.UTC(2026, 10, 2), journalDate: "2026-11-02" }, [{ ...GIF, description: "my cat", name: "cat.gif" }]);
    // The same content again, a covered GIF and a plain photo are not offered.
    api.addMessageWithAttachments({ id: "m3", bodyMd: "", createdAt: Date.UTC(2026, 10, 3), journalDate: "2026-11-03" }, [{ ...GIF, id: "gif-dup" }]);
    api.addMessageWithAttachments({ id: "m4", bodyMd: "", createdAt: Date.UTC(2026, 10, 4), journalDate: "2026-11-04" }, [{ ...GIF, id: "gif-hidden", filename: "h.gif", spoiler: true }]);
    api.addMessageWithAttachments({ id: "m5", bodyMd: "", createdAt: Date.UTC(2026, 10, 5), journalDate: "2026-11-05" }, [demoPhoto("p1", "p.jpg", 100, 100)]);
    const gifs = await api.listJournalGifs();
    expect(gifs.map((gif) => gif.attachmentId)).toEqual(["gif-dup", "gif-old"]);
    expect([gifs[0]?.width, gifs[0]?.height, gifs[0]?.byteSize]).toEqual([480, 270, 1_000_000]);

    const sent = await api.sendGifAgain(INBOX_ID, "gif-dup");
    expect(sent.bodyMd).toBe("");
    expect(sent.attachments[0]).toMatchObject({ mime: "image/gif", sticker: false, description: null, spoiler: false });
    expect(sent.attachments[0]?.id).not.toBe("gif-dup");
  });

  it("a covered, unknown or non-GIF source is attachment_not_found; an unknown chat is chat_not_found", async () => {
    const { api } = setup();
    api.addMessageWithAttachments({ id: "m1", bodyMd: "" }, [{ ...GIF, spoiler: true }]);
    api.addMessageWithAttachments({ id: "m2", bodyMd: "" }, [demoPhoto("p1", "p.jpg", 100, 100)]);
    await expect(api.sendGifAgain(INBOX_ID, GIF.id)).rejects.toMatchObject({ code: "attachment_not_found" });
    await expect(api.sendGifAgain(INBOX_ID, "p1")).rejects.toMatchObject({ code: "attachment_not_found" });
    await expect(api.sendGifAgain(INBOX_ID, "nope")).rejects.toMatchObject({ code: "attachment_not_found" });
    await expect(api.sendGifAgain("no-chat", "p1")).rejects.toMatchObject({ code: "chat_not_found" });
  });

  it("a trashed GIF is not offered", async () => {
    const { api } = setup();
    const message = mockMessage({ id: "m1", bodyMd: "", attachments: [GIF] });
    api.addExternalMessage(message);
    expect(await api.listJournalGifs()).toHaveLength(1);
    await api.deleteMessage("m1");
    expect(await api.listJournalGifs()).toEqual([]);
  });
});

describe("MockApi: the bot token", () => {
  it("saves a token of the right shape, reads back only the flag, removes it", async () => {
    const { api } = setup();
    expect(await api.getTelegramTokenState()).toEqual({ saved: false, itemLeft: false });
    expect(await api.setTelegramBotToken(" 123456:ABC-def_1 ")).toEqual({ saved: true, itemLeft: false });
    expect(await api.getTelegramTokenState()).toEqual({ saved: true, itemLeft: false });
    expect((await api.getStickerLibrary()).hasToken).toBe(true);
    expect(await api.clearTelegramBotToken()).toEqual({ saved: false, itemLeft: false });
    // The token is never in the call log.
    expect(api.calls.join("\n")).not.toContain("ABC-def");
  });

  it("refuses a token of the wrong shape", async () => {
    const { api } = setup();
    for (const bad of ["", "abc", "123456", "123456:", ":abc", "12 34:abc", `${"9".repeat(21)}:abc`, `1:${"x".repeat(101)}`, "1:a/b"]) {
      await expect(api.setTelegramBotToken(bad), bad).rejects.toMatchObject({ code: "telegram_token_invalid" });
    }
    expect(await api.getTelegramTokenState()).toEqual({ saved: false, itemLeft: false });
  });
});
