import { KinigeApiError } from "./KinigeApi";
import { MockApiSlice6 } from "./MockApiSlice6";
import { mockMessage } from "./mockData";
import type { AttachOutcome } from "./generated/AttachOutcome";
import type { AttachmentView } from "./generated/AttachmentView";
import type { ChatDraft } from "./generated/ChatDraft";
import type { MessageView } from "./generated/MessageView";

/** The most attachments in one entry and the largest paste (like `kinige_domain::attachment`). */
const MAX_PER_ENTRY = 10;
const MAX_PASTE_BYTES = 64 * 1024 * 1024;

/** The picture size in pixels the mock gives a pasted or picked image. */
const PICTURE_W = 640;
const PICTURE_H = 480;

/** A tile for a file the test "attached": a photo if the name says so, a file card otherwise. */
export function mockAttachment(id: string, filename: string, part: Partial<AttachmentView> = {}): AttachmentView {
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";
  const image = ["jpg", "jpeg", "png", "gif", "webp"].includes(ext);
  const video = ["mp4", "mov", "webm"].includes(ext);
  const audio = ["mp3", "m4a", "ogg", "wav"].includes(ext);
  return {
    id,
    kind: image ? "image" : video ? "video" : audio ? "audio" : "file",
    filename,
    mime: image ? `image/${ext === "jpg" ? "jpeg" : ext}` : "application/octet-stream",
    byteSize: 2048,
    width: image ? PICTURE_W : null,
    height: image ? PICTURE_H : null,
    durationMs: null,
    state: "local",
    ...part,
  };
}

/**
 * Slice 7 in `MockApi`: draft attachments (the tiles above the field), sending with them, a paste, a drop and the system
 * dialog. The real rules (formats, limits, files) are in Rust; here is what tests and stories need: the same error codes and the
 * same order of events.
 */
export class MockApiSlice7 extends MockApiSlice6 {
  /** What the system file dialog will "pick" next (names); cleared after use. `null`: the user cancels. */
  pickedFiles: string[] | null = null;
  /** The tiles by chat, in order. */
  protected readonly draftTiles = new Map<string, AttachmentView[]>();
  /** What a drop token claims (set by `emitAttachmentsDropped`). */
  private readonly drops = new Map<string, string[]>();
  /** Attachments opened in the system app. */
  readonly openedAttachments: string[] = [];
  private readonly dragListeners = new Set<(over: boolean) => void>();
  private readonly dropListeners = new Set<(token: string) => void>();

  /** The tiles above the field of a chat. */
  tilesOf(chatId: string): AttachmentView[] {
    return [...(this.draftTiles.get(chatId) ?? [])];
  }

  sendMessage(chatId: string, body: string, replyToId: string | null = null, attachmentIds: readonly string[] = []): Promise<MessageView> {
    if (attachmentIds.length === 0) return super.sendMessage(chatId, body, replyToId);
    if (this.failNextSend) return super.sendMessage(chatId, body, replyToId);
    const tiles = this.draftTiles.get(chatId) ?? [];
    if (attachmentIds.length > MAX_PER_ENTRY) return Promise.reject(new KinigeApiError("attachment_limit"));
    const chosen: AttachmentView[] = [];
    for (const id of attachmentIds) {
      const tile = tiles.find((t) => t.id === id);
      if (!tile || chosen.includes(tile)) return Promise.reject(new KinigeApiError("attachment_not_found"));
      chosen.push(tile);
    }
    const text = body.replace(/\r\n?/g, "\n").trim();
    // The send itself is the slice 5 path (a reply, the draft cleared); the text may be empty only with attachments.
    const made = super.sendMessage(chatId, text === "" ? "." : text, replyToId);
    return made.then((sent) => {
      const row = this.row(sent.id);
      row.view = { ...row.view, bodyMd: text, tags: [], attachments: chosen };
      this.draftTiles.set(chatId, tiles.filter((t) => !chosen.includes(t)));
      return this.viewOf(row);
    });
  }

  getDraft(chatId: string): Promise<ChatDraft> {
    return super.getDraft(chatId).then((draft) => ({ ...draft, attachments: this.tilesOf(chatId) }));
  }

  attachPick(chatId: string): Promise<AttachOutcome> {
    return this.run("attachPick", chatId, () => {
      const names = this.pickedFiles;
      this.pickedFiles = null;
      return this.attachNames(chatId, names ?? []);
    });
  }

  attachPaste(chatId: string, bytes: Uint8Array, nameHint?: string): Promise<AttachmentView> {
    return this.run("attachPaste", `${chatId} ${bytes.length} ${nameHint ?? ""}`, () => {
      if (bytes.length > MAX_PASTE_BYTES) throw new KinigeApiError("attachment_too_large");
      const name = nameHint && nameHint.trim() !== "" ? nameHint : "pasted-120000.png";
      return this.addTile(chatId, name, bytes.length);
    });
  }

  attachDropped(token: string, chatId: string): Promise<AttachOutcome> {
    return this.run("attachDropped", `${token} ${chatId}`, () => {
      const names = this.drops.get(token);
      if (!names) throw new KinigeApiError("attachment_not_found");
      this.drops.delete(token);
      return this.attachNames(chatId, names);
    });
  }

  removeDraftAttachment(chatId: string, attachmentId: string): Promise<void> {
    return this.run("removeDraftAttachment", `${chatId} ${attachmentId}`, () => {
      const tiles = this.draftTiles.get(chatId) ?? [];
      if (!tiles.some((t) => t.id === attachmentId)) throw new KinigeApiError("attachment_not_found");
      this.draftTiles.set(chatId, tiles.filter((t) => t.id !== attachmentId));
    });
  }

  openAttachment(attachmentId: string): Promise<void> {
    return this.run("openAttachment", attachmentId, () => {
      this.openedAttachments.push(attachmentId);
    });
  }

  onAttachmentsDrag(handler: (over: boolean) => void): Promise<() => void> {
    this.dragListeners.add(handler);
    return Promise.resolve(() => {
      this.dragListeners.delete(handler);
    });
  }

  onAttachmentsDropped(handler: (token: string) => void): Promise<() => void> {
    this.dropListeners.add(handler);
    return Promise.resolve(() => {
      this.dropListeners.delete(handler);
    });
  }

  /** Like the Rust events while files are dragged over the window. */
  emitAttachmentsDrag(over: boolean): void {
    for (const handler of [...this.dragListeners]) handler(over);
  }

  /** Like a drop of the named files on the window: the token the window claims with `attachDropped`. */
  emitAttachmentsDropped(names: string[], token = `drop-${++this.counter}`): string {
    this.drops.set(token, names);
    for (const handler of [...this.dropListeners]) handler(token);
    return token;
  }

  /** An entry that already has attachments (for feed tests): ids are stable, the files are "present". */
  addMessageWithAttachments(part: Partial<MessageView> & { id: string; bodyMd: string }, attachments: AttachmentView[]): MessageView {
    const message = mockMessage({ ...part, attachments });
    this.addExternalMessage(message);
    return message;
  }

  private attachNames(chatId: string, names: string[]): AttachOutcome {
    const outcome: AttachOutcome = { attached: [], skippedFolders: 0, error: null };
    for (const name of names) {
      if (name.endsWith("/")) {
        outcome.skippedFolders += 1;
        continue;
      }
      try {
        outcome.attached.push(this.addTile(chatId, name, 2048));
      } catch (error) {
        outcome.error = error instanceof KinigeApiError ? error.code : "internal";
        break;
      }
    }
    if (outcome.error === null && outcome.skippedFolders > 0) outcome.error = "folders_not_attached";
    return outcome;
  }

  private addTile(chatId: string, name: string, byteSize: number): AttachmentView {
    if (!this.chats.some((c) => c.id === chatId)) throw new KinigeApiError("chat_not_found");
    const tiles = this.draftTiles.get(chatId) ?? [];
    if (tiles.length >= MAX_PER_ENTRY) throw new KinigeApiError("attachment_limit");
    const tile = mockAttachment(`att-${++this.counter}`, name, { byteSize });
    this.draftTiles.set(chatId, [...tiles, tile]);
    return tile;
  }
}
