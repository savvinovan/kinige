import { describe, expect, it } from "vitest";
import { fixture } from "./apiContractFixtures";
import { INBOX_ID, errorCode } from "./KinigeApi";
import { MockApi } from "./MockApi";

// The scenarios of `fixtures/api-contract/`: the same steps that `crates/kinige-store/tests/api_contract/flow.rs` runs on a real `Store`
// run here on `MockApi`. A step with `error` must fail with that code, any other step must succeed.

interface Step {
  op: string;
  error?: string;
  [field: string]: unknown;
}

interface Scenarios {
  scenarios: { name: string; steps: Step[] }[];
}

const FILES = ["reply-flow", "page-limit", "attachment-errors", "recent-emoji"];

describe("fixtures/api-contract: the scenarios on MockApi", () => {
  for (const file of FILES) {
    describe(file, () => {
      for (const scenario of fixture<Scenarios>(file).scenarios) {
        it(scenario.name, async () => {
          const world = new World();
          for (const step of scenario.steps) {
            const failure = await world.run(step);
            expect(failure, JSON.stringify(step)).toEqual(step.error ?? null);
          }
        });
      }
    });
  }
});

/** The mock, the ids that labels stand for, and the chat each entry was sent to. */
class World {
  private readonly api = new MockApi();
  private readonly labels = new Map<string, string>([["inbox", INBOX_ID]]);
  private readonly chatOf = new Map<string, string>();

  /** Runs one step. Returns the error code it failed with, or `null`; a failed check throws. */
  async run(step: Step): Promise<string | null> {
    try {
      await this.act(step);
      return null;
    } catch (error) {
      if (error instanceof Error && error.name === "KinigeApiError") return errorCode(error);
      throw error;
    }
  }

  private async act(step: Step): Promise<void> {
    const api = this.api;
    const chat = this.id(step.chat);
    const replyTo = typeof step.replyTo === "string" ? this.id(step.replyTo) : null;
    switch (step.op) {
      case "chat":
        this.label(step.as, (await api.createChat(String(step.as), null)).id);
        break;
      case "send": {
        const sent = await api.sendMessage(chat, String(step.body), replyTo);
        this.sent(step.as, chat, sent.id);
        break;
      }
      case "seed":
        for (let n = 0; n < Number(step.count); n++) await api.sendMessage(chat, `entry ${n}`);
        break;
      case "page": {
        const limit = typeof step.limit === "number" ? step.limit : undefined;
        expect((await api.listMessages(chat, { limit })).messages).toHaveLength(Number(step.count));
        break;
      }
      case "search": {
        const filters = { chatId: null, favorite: false, tasks: false, from: (step.from as string) ?? null, to: (step.to as string) ?? null };
        const limit = typeof step.limit === "number" ? step.limit : undefined;
        expect((await api.searchMessages(String(step.query), { filters, limit })).hits).toHaveLength(Number(step.count));
        break;
      }
      case "saveDraft":
        await api.saveDraft(chat, String(step.body), replyTo);
        break;
      case "draft": {
        const draft = await api.getDraft(chat);
        expect(draft.body).toBe(step.body);
        expect(draft.replyTo?.state ?? null).toBe(step.quote);
        break;
      }
      case "feedQuote":
        await this.checkFeedQuote(step);
        break;
      case "move":
        await api.moveMessage(this.id(step.msg), this.id(step.to));
        break;
      case "delete":
        await api.deleteMessage(this.id(step.msg));
        break;
      case "restore":
        await api.restoreMessage(this.id(step.msg));
        break;
      case "purge":
        await api.purgeMessage(this.id(step.msg));
        break;
      case "paste": {
        const bytes = new Uint8Array(typeof step.bytes === "number" ? step.bytes : 16);
        this.label(step.as, (await api.attachPaste(chat, bytes)).id);
        break;
      }
      case "removeTile":
        await api.removeDraftAttachment(chat, this.id(step.tile));
        break;
      case "tiles":
        expect((await api.getDraft(chat)).attachments).toHaveLength(Number(step.count));
        break;
      case "sendWith": {
        const ids = (step.ids as string[]).map((tile) => this.id(tile));
        this.sent(step.as, chat, (await api.sendMessage(chat, String(step.body), replyTo, ids)).id);
        break;
      }
      case "voice": {
        const bytes = new Uint8Array(typeof step.bytes === "number" ? step.bytes : 4096);
        this.sent(step.as, chat, (await api.sendVoice(chat, bytes, Number(step.durationMs), replyTo)).id);
        break;
      }
      case "recent":
        this.checkRecent(step, await api.addRecentEmoji(String(step.emoji)));
        break;
      case "recentMany": {
        let list: string[] = [];
        for (const emoji of step.emojis as string[]) list = await api.addRecentEmoji(emoji);
        this.checkRecent(step, list);
        break;
      }
      default:
        throw new Error(`unknown step ${step.op}`);
    }
  }

  /** The quote under a reply, read from the feed of the chat the reply was sent to. */
  private async checkFeedQuote(step: Step): Promise<void> {
    const message = this.id(step.msg);
    const page = await this.api.listMessages(this.chatOf.get(message) ?? "");
    const quote = page.messages.find((m) => m.id === message)?.replyTo;
    expect(quote?.state).toBe(step.state);
    expect(quote?.chatId).toBe(typeof step.quoteChat === "string" ? this.id(step.quoteChat) : null);
  }

  private checkRecent(step: Step, list: string[]): void {
    if (Array.isArray(step.list)) expect(list).toEqual(step.list);
    if (typeof step.count === "number") expect(list).toHaveLength(step.count);
    if (typeof step.first === "string") expect(list[0]).toBe(step.first);
    if (typeof step.last === "string") expect(list.at(-1)).toBe(step.last);
  }

  /** The id a label stands for; an unknown label stands for itself. */
  private id(label: unknown): string {
    return typeof label === "string" ? (this.labels.get(label) ?? label) : "";
  }

  private label(name: unknown, id: string): void {
    if (typeof name === "string") this.labels.set(name, id);
  }

  private sent(name: unknown, chat: string, id: string): void {
    this.chatOf.set(id, chat);
    this.label(name, id);
  }
}
