import { KinigeApiError } from "../KinigeApi";
import { mockMessage } from "../mockData";
import { LIMITS, attachmentKind, storedExt } from "../mockRules";
import type { AttachOutcome } from "../generated/AttachOutcome";
import type { AttachmentView } from "../generated/AttachmentView";
import type { MessageView } from "../generated/MessageView";
import type { MockState } from "./MockState";

/** The picture size in pixels the mock gives a pasted or picked image. */
const PICTURE_W = 640;
const PICTURE_H = 480;

/** A tile for a file the test "attached": a photo if the name says so, a file card otherwise (`fixtures/api-contract/attachment-kind.json`). */
export function mockAttachment(id: string, filename: string, part: Partial<AttachmentView> = {}): AttachmentView {
  const kind = attachmentKind(filename);
  const ext = storedExt(filename);
  const image = kind === "image";
  return {
    id,
    kind,
    filename,
    mime: image ? `image/${ext === "jpg" ? "jpeg" : ext}` : "application/octet-stream",
    byteSize: 2048,
    width: image ? PICTURE_W : null,
    height: image ? PICTURE_H : null,
    durationMs: null,
    state: "local",
    waveform: null,
    ...part,
  };
}

/**
 * Draft attachments in `MockApi` (slice 7): the tiles above the field, a paste, a drop, the system dialog, opening a file. The real
 * rules (formats, files) are in Rust; here is what tests and stories need: the same limits and error codes
 * (`fixtures/api-contract/attachment-errors.json`) and the same order of events. Sending an entry with the tiles is in `MockCompose`.
 */
export class MockAttachments {
  /** What the system file dialog will "pick" next (names); cleared after use. `null`: the user cancels. */
  pickedFiles: string[] | null = null;
  /** Attachments opened in the system app. */
  readonly openedAttachments: string[] = [];
  /** What a drop token claims (set by `emitAttachmentsDropped`). */
  private readonly drops = new Map<string, string[]>();
  private readonly dragListeners = new Set<(over: boolean) => void>();
  private readonly dropListeners = new Set<(token: string) => void>();

  constructor(private readonly s: MockState) {}

  /** The tiles above the field of a chat. */
  tilesOf(chatId: string): AttachmentView[] {
    return [...(this.s.draftTiles.get(chatId) ?? [])];
  }

  attachPick(chatId: string): Promise<AttachOutcome> {
    return this.s.run("attachPick", chatId, () => {
      const names = this.pickedFiles;
      this.pickedFiles = null;
      return this.attachNames(chatId, names ?? []);
    });
  }

  attachPaste(chatId: string, bytes: Uint8Array, nameHint?: string): Promise<AttachmentView> {
    return this.s.run("attachPaste", `${chatId} ${bytes.length} ${nameHint ?? ""}`, () => {
      if (bytes.length > LIMITS.pasteMaxBytes) throw new KinigeApiError("attachment_too_large");
      const name = nameHint && nameHint.trim() !== "" ? nameHint : "pasted-120000.png";
      return this.addTile(chatId, name, bytes.length);
    });
  }

  attachDropped(token: string, chatId: string): Promise<AttachOutcome> {
    return this.s.run("attachDropped", `${token} ${chatId}`, () => {
      const names = this.drops.get(token);
      if (!names) throw new KinigeApiError("attachment_not_found");
      this.drops.delete(token);
      return this.attachNames(chatId, names);
    });
  }

  removeDraftAttachment(chatId: string, attachmentId: string): Promise<void> {
    return this.s.run("removeDraftAttachment", `${chatId} ${attachmentId}`, () => {
      const tiles = this.s.draftTiles.get(chatId) ?? [];
      if (!tiles.some((t) => t.id === attachmentId)) throw new KinigeApiError("attachment_not_found");
      this.s.draftTiles.set(chatId, tiles.filter((t) => t.id !== attachmentId));
    });
  }

  openAttachment(attachmentId: string): Promise<void> {
    return this.s.run("openAttachment", attachmentId, () => {
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
  emitAttachmentsDropped(names: string[], token = `drop-${++this.s.counter}`): string {
    this.drops.set(token, names);
    for (const handler of [...this.dropListeners]) handler(token);
    return token;
  }

  /** An entry that already has attachments (for feed tests): ids are stable, the files are "present". */
  addMessageWithAttachments(part: Partial<MessageView> & { id: string; bodyMd: string }, attachments: AttachmentView[]): MessageView {
    const message = mockMessage({ ...part, attachments });
    this.s.rows.push({ view: message, deletedAt: null, revisions: [] });
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
    if (!this.s.chats.some((c) => c.id === chatId)) throw new KinigeApiError("chat_not_found");
    const tiles = this.s.draftTiles.get(chatId) ?? [];
    if (tiles.length >= LIMITS.attachmentsPerEntry) throw new KinigeApiError("attachment_limit");
    const tile = mockAttachment(`att-${++this.s.counter}`, name, { byteSize });
    this.s.draftTiles.set(chatId, [...tiles, tile]);
    return tile;
  }
}
