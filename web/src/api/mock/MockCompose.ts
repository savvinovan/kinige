import { KinigeApiError } from "../KinigeApi";
import { mockMessage } from "../mockData";
import { LIMITS } from "../mockRules";
import { mockTags } from "../mockTags";
import type { AttachmentView } from "../generated/AttachmentView";
import type { ChatDraft } from "../generated/ChatDraft";
import type { ErrorCode } from "../generated/ErrorCode";
import type { MessageView } from "../generated/MessageView";
import type { MockState } from "./MockState";

/** Writing in `MockApi`: sending an entry (with a reply, with attachments), and the chat draft with its reply line and tiles. */
export class MockCompose {
  /** The next send fails with this error without writing anything. */
  failNextSend: ErrorCode | null = null;

  constructor(private readonly s: MockState) {}

  /** The checks run in Rust's order: empty text, the reply target, the chat, then the draft's tiles (their number, then each id). */
  sendMessage(chatId: string, body: string, replyToId: string | null = null, attachmentIds: readonly string[] = []): Promise<MessageView> {
    try {
      return Promise.resolve(this.deliver(chatId, body, replyToId, (tiles) => this.chosen(tiles, attachmentIds), attachmentIds.length > 0));
    } catch (error) {
      return Promise.reject(error);
    }
  }

  getDraft(chatId: string): Promise<ChatDraft> {
    const target = this.s.replyDrafts.get(chatId);
    let replyTo = target === undefined ? null : this.s.quote(target, this.s.journalDate(this.s.now()));
    // As in Rust: the entry is alive but in another chat - "moved" (a reply to it is not accepted).
    if (replyTo !== null && replyTo.state === "live" && replyTo.chatId !== chatId) replyTo = { ...replyTo, state: "moved" };
    return Promise.resolve({ body: this.s.drafts.get(chatId) ?? "", replyTo, attachments: [...(this.s.draftTiles.get(chatId) ?? [])] });
  }

  saveDraft(chatId: string, body: string, replyToId: string | null = null): Promise<void> {
    if (!this.s.chats.some((c) => c.id === chatId)) return Promise.reject(new KinigeApiError("chat_not_found"));
    if (replyToId !== null && replyToId === "") return Promise.reject(new KinigeApiError("reply_target_not_found"));
    if (body === "") this.s.drafts.delete(chatId);
    else this.s.drafts.set(chatId, body);
    if (replyToId === null) this.s.replyDrafts.delete(chatId);
    else this.s.replyDrafts.set(chatId, replyToId);
    return Promise.resolve();
  }

  /**
   * Writes the entry: `attach` picks its attachments from the chat's tiles (and may refuse). Sending clears this chat's draft with its
   * reply line (as in Rust). The text may be empty only with attachments (`withAttachments`).
   */
  deliver(
    chatId: string,
    body: string,
    replyToId: string | null,
    attach: (tiles: AttachmentView[]) => AttachmentView[],
    withAttachments: boolean,
  ): MessageView {
    if (this.failNextSend) {
      const code = this.failNextSend;
      this.failNextSend = null;
      throw new KinigeApiError(code);
    }
    const text = body.replace(/\r\n?/g, "\n").trim();
    if (text === "" && !withAttachments) throw new KinigeApiError("empty_message");
    if (replyToId !== null && !this.replyTargetLives(replyToId, chatId)) throw new KinigeApiError("reply_target_not_found");
    if (!this.s.chats.some((c) => c.id === chatId)) throw new KinigeApiError("chat_not_found");
    const tiles = this.s.draftTiles.get(chatId) ?? [];
    const attachments = attach(tiles);
    const now = this.s.now();
    const message = mockMessage({
      id: `mock-${++this.s.counter}`,
      chatId,
      bodyMd: text,
      journalDate: this.s.journalDate(now),
      createdAt: now.getTime(),
      tags: mockTags(text),
      attachments,
    });
    const row = { view: message, deletedAt: null, revisions: [], replyToId: replyToId ?? undefined };
    this.s.rows.push(row);
    if (attachments.length > 0) this.s.draftTiles.set(chatId, tiles.filter((t) => !attachments.includes(t)));
    this.s.drafts.delete(chatId);
    this.s.replyDrafts.delete(chatId);
    return this.s.viewOf(row);
  }

  /** The draft's own tiles named by `ids`: at most ten, each once, in the order given. */
  private chosen(tiles: AttachmentView[], ids: readonly string[]): AttachmentView[] {
    if (ids.length > LIMITS.attachmentsPerEntry) throw new KinigeApiError("attachment_limit");
    const chosen: AttachmentView[] = [];
    for (const id of ids) {
      const tile = tiles.find((t) => t.id === id);
      if (!tile || chosen.includes(tile)) throw new KinigeApiError("attachment_not_found");
      chosen.push(tile);
    }
    return chosen;
  }

  /** The reply addressee is alive and lies in the same chat. */
  private replyTargetLives(targetId: string, chatId: string): boolean {
    return this.s.rows.some((r) => r.view.id === targetId && r.view.chatId === chatId && r.deletedAt === null);
  }
}
