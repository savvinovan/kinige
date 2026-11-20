import { KinigeApiError } from "../KinigeApi";
import { demoPhoto } from "../../ui/attachments/demo";
import { registerDemoSticker } from "../../ui/stickers/stickerArt";
import { MAX_RECENT_STICKERS, isBotToken, isLibraryId, packName } from "../mockStickerRules";
import type { AttachmentView } from "../generated/AttachmentView";
import type { ErrorCode } from "../generated/ErrorCode";
import type { JournalGif } from "../generated/JournalGif";
import type { MessageView } from "../generated/MessageView";
import type { StickerLibrary } from "../generated/StickerLibrary";
import type { StickerPackAdded } from "../generated/StickerPackAdded";
import type { StickerPackProgress } from "../generated/StickerPackProgress";
import type { StickerPackView } from "../generated/StickerPackView";
import type { StickerView } from "../generated/StickerView";
import type { TelegramTokenState } from "../generated/TelegramTokenState";
import type { MockCompose } from "./MockCompose";
import type { MockState } from "./MockState";

/** A pack the pretend Telegram knows by name: how many static stickers it has, how many it skips, or the error the download ends with. */
export interface MockRemotePack {
  title: string;
  stickers: number;
  skipped?: number;
  error?: ErrorCode;
}

const EMOJI = ["\u{1F63A}", "\u{1F431}", "\u{1F436}", "\u{1F98A}", "\u{1F43C}", "\u{1F984}", "\u{1F438}", "\u{1F419}", "\u{1F422}", "\u{1F989}"];
/** The side of a synthetic sticker, in pixels (a static sticker has one side of 512). */
const SIDE = 512;
/** The synthetic GIF's own sticker-like size is its attachment's; this is the byte size a sticker of the mock claims. */
const STICKER_BYTES = 12_000;

/**
 * Stickers, GIFs and the bot token in `MockApi` (slice 5c). The library is in memory, its pictures are drawn by code (`stickerArt`), and the
 * pretend Telegram answers for the packs in `remotePacks`. The refusals are Rust's: ids that are not UUIDs are no sticker, the chat is checked
 * before the sticker, one pack job at a time, a cancelled job adds nothing. A sticker entry is never edited, has no details and is not an
 * append target (that part lives in `MockEntries`, `MockAttachments` and `MockCapture`, through `stickerOf`).
 */
export class MockStickers {
  /** Whether a bot token is saved (the flag the window sees; the token itself is never kept). */
  hasToken = false;
  /** The system refuses to delete the saved item (the prompt was denied): Remove then answers `itemLeft`. */
  denyDelete = false;
  /** The packs "Telegram" knows by name; the default one is a typical pack with a few animated ones skipped. */
  readonly remotePacks = new Map<string, MockRemotePack>([["TestPack", { title: "Test Pack", stickers: 27, skipped: 3 }]]);
  /** How many progress events a pack job sends (the last one is "all"). */
  steps = 3;
  /** The job freezes after the first progress event until `release()` (or `cancelStickerPack`). */
  hold = false;
  /** Library ids whose file "fails the probe again" at send time. */
  readonly unreadable = new Set<string>();
  /** The pack names asked for, in order (what the fake server would have seen, without the token). */
  readonly requested: string[] = [];

  private packs: StickerPackView[] = [];
  /** The link name of each pack added by link (what a second add of the same name replaces). */
  private readonly linkNames = new Map<string, string>();
  private recentIds: string[] = [];
  private counter = 0;
  private running = false;
  private cancelled = false;
  private releaseHold: (() => void) | null = null;
  private readonly progressListeners = new Set<(progress: StickerPackProgress) => void>();

  constructor(
    private readonly s: MockState,
    private readonly compose: MockCompose,
  ) {}

  /** Puts a pack into the library (stories and tests): the export's pack first, the rest in the order added. Returns it. */
  seedPack(title: string, count: number, source: StickerPackView["source"] = "telegram_link"): StickerPackView {
    const pack: StickerPackView = {
      id: this.uuid(),
      source,
      title,
      stickers: Array.from({ length: count }, (_, index) => this.newSticker(EMOJI[index % EMOJI.length] ?? null)),
    };
    this.packs = source === "telegram_export" ? [pack, ...this.packs] : [...this.packs, pack];
    return pack;
  }

  /** A sticker as recently sent (stories). */
  seedRecent(stickerIds: readonly string[]): void {
    this.recentIds = stickerIds.filter((id) => this.find(id) !== undefined).slice(0, MAX_RECENT_STICKERS);
  }

  getStickerLibrary(): Promise<StickerLibrary> {
    return this.s.run("getStickerLibrary", "", () => ({
      recent: this.recentIds.flatMap((id) => {
        const sticker = this.find(id);
        return sticker ? [sticker] : [];
      }),
      // As in Rust: a pack with no sticker is not shown.
      packs: this.packs.filter((pack) => pack.stickers.length > 0),
      hasToken: this.hasToken,
    }));
  }

  async addStickerPack(link: string): Promise<StickerPackAdded> {
    this.s.calls.push(`addStickerPack ${link}`);
    const code = this.s.takeFailure("addStickerPack");
    if (code) throw new KinigeApiError(code);
    if (this.running) throw new KinigeApiError("sticker_pack_busy");
    const name = packName(link);
    if (name === null) throw new KinigeApiError("sticker_pack_link_invalid");
    // No token, no request: the job stops before it asks Telegram anything.
    if (!this.hasToken) throw new KinigeApiError("sticker_pack_no_token");
    this.requested.push(name);
    const remote = this.remotePacks.get(name);
    if (!remote) throw new KinigeApiError("sticker_pack_not_found");
    this.running = true;
    this.cancelled = false;
    try {
      const total = remote.stickers + (remote.skipped ?? 0);
      const steps = Math.max(1, this.steps);
      for (let step = 1; step <= steps; step++) {
        if (this.cancelled) throw new KinigeApiError("sticker_pack_cancelled");
        const done = Math.round((total * step) / steps);
        for (const handler of [...this.progressListeners]) handler({ done, total });
        if (step < steps && this.hold) await new Promise<void>((resolve) => (this.releaseHold = resolve));
      }
      if (this.cancelled) throw new KinigeApiError("sticker_pack_cancelled");
      if (remote.error) throw new KinigeApiError(remote.error);
      if (remote.stickers === 0) throw new KinigeApiError("sticker_pack_only_animated");
      // A name already in the library replaces that pack.
      const old = this.packs.find((pack) => this.linkNames.get(pack.id) === name);
      if (old) this.packs = this.packs.filter((pack) => pack !== old);
      this.linkNames.set(this.seedPack(remote.title, remote.stickers).id, name);
      return { title: remote.title, added: remote.stickers, skipped: remote.skipped ?? 0 };
    } finally {
      this.running = false;
    }
  }

  cancelStickerPack(): Promise<void> {
    return this.s.run("cancelStickerPack", "", () => {
      this.cancelled = this.running;
      this.release();
    });
  }

  removeStickerPack(packId: string): Promise<void> {
    return this.s.run("removeStickerPack", packId, () => {
      if (!isLibraryId(packId) || !this.packs.some((pack) => pack.id === packId)) throw new KinigeApiError("sticker_not_found");
      this.packs = this.packs.filter((pack) => pack.id !== packId);
    });
  }

  sendSticker(chatId: string, stickerId: string, replyToId: string | null = null): Promise<MessageView> {
    return this.s.run("sendSticker", `${chatId} ${stickerId} ${replyToId ?? ""}`, () => {
      // Rust's order: the id's shape, the chat, the library row, the probe, then the reply target in the write.
      if (!isLibraryId(stickerId)) throw new KinigeApiError("sticker_not_found");
      this.s.chat(chatId);
      const sticker = this.find(stickerId);
      if (!sticker) throw new KinigeApiError("sticker_not_found");
      if (this.unreadable.has(stickerId)) throw new KinigeApiError("sticker_unreadable");
      const attachment = demoPhoto(`att-${++this.s.counter}`, "sticker.webp", sticker.width, sticker.height, {
        sticker: { emoji: sticker.emoji },
        byteSize: STICKER_BYTES,
      });
      const sent = this.compose.deliver(chatId, "", replyToId, () => [attachment], true, true);
      this.recentIds = [stickerId, ...this.recentIds.filter((id) => id !== stickerId)].slice(0, MAX_RECENT_STICKERS);
      return sent;
    });
  }

  listJournalGifs(): Promise<JournalGif[]> {
    return this.s.run("listJournalGifs", "", () => {
      const seen = new Set<string>();
      const gifs: JournalGif[] = [];
      // Newest first, one per content (the mock's stand-in for the hash is the name and size), live and uncovered only.
      for (const message of this.s.allLive().reverse()) {
        for (const attachment of message.attachments) {
          const key = `${attachment.filename}|${attachment.byteSize}`;
          if (!isGif(attachment) || attachment.spoiler || seen.has(key) || attachment.width === null || attachment.height === null) continue;
          seen.add(key);
          gifs.push({ attachmentId: attachment.id, width: attachment.width, height: attachment.height, byteSize: attachment.byteSize });
        }
      }
      return gifs;
    });
  }

  sendGifAgain(chatId: string, attachmentId: string, replyToId: string | null = null): Promise<MessageView> {
    return this.s.run("sendGifAgain", `${chatId} ${attachmentId} ${replyToId ?? ""}`, () => {
      this.s.chat(chatId);
      const source = this.s.allLive().flatMap((message) => message.attachments).find((attachment) => attachment.id === attachmentId);
      if (!source || !isGif(source) || source.spoiler) throw new KinigeApiError("attachment_not_found");
      // The copy starts without details (no name, description or cover of the source) and has its own id.
      const copy: AttachmentView = { ...source, id: `att-${++this.s.counter}`, name: source.filename, description: null, spoiler: false };
      return this.compose.deliver(chatId, "", replyToId, () => [copy], true, true);
    });
  }

  onStickerPackProgress(handler: (progress: StickerPackProgress) => void): Promise<() => void> {
    this.progressListeners.add(handler);
    return Promise.resolve(() => {
      this.progressListeners.delete(handler);
    });
  }

  /** Continues a job frozen by `hold` up to its next progress event. */
  release(): void {
    this.releaseHold?.();
    this.releaseHold = null;
  }

  getTelegramTokenState(): Promise<TelegramTokenState> {
    return this.s.run("getTelegramTokenState", "", () => ({ saved: this.hasToken, itemLeft: false }));
  }

  setTelegramBotToken(token: string): Promise<TelegramTokenState> {
    // The token is not logged in the call log, as Rust never logs it.
    return this.s.run("setTelegramBotToken", "", () => {
      if (!isBotToken(token.trim())) throw new KinigeApiError("telegram_token_invalid");
      this.hasToken = true;
      return { saved: true, itemLeft: false };
    });
  }

  /** Remove: Rust always clears the flag; when the system refuses the delete (`denyDelete`) the answer says the old item is left. */
  clearTelegramBotToken(): Promise<TelegramTokenState> {
    return this.s.run("clearTelegramBotToken", "", () => {
      this.hasToken = false;
      return { saved: false, itemLeft: this.denyDelete };
    });
  }

  private find(id: string): StickerView | undefined {
    for (const pack of this.packs) {
      const sticker = pack.stickers.find((candidate) => candidate.id === id);
      if (sticker) return sticker;
    }
    return undefined;
  }

  private newSticker(emoji: string | null): StickerView {
    const id = this.uuid();
    registerDemoSticker(id, emoji);
    return { id, emoji, width: SIDE, height: SIDE };
  }

  /** A version 4 UUID that is the same in every run. */
  private uuid(): string {
    this.counter += 1;
    return `5c000000-0000-4000-8000-${this.counter.toString(16).padStart(12, "0")}`;
  }
}

const isGif = (attachment: AttachmentView) => attachment.kind === "image" && attachment.mime.toLowerCase() === "image/gif";
