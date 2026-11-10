import { type ListMessagesOptions, KinigeApiError } from "../KinigeApi";
import { compareFeed } from "../mockData";
import { isRealDate, pageLimit } from "../mockRules";
import { toggleTaskItem } from "../mockTasks";
import type { FeedCursor } from "../generated/FeedCursor";
import type { MessageFlag } from "../generated/MessageFlag";
import type { MessagePage } from "../generated/MessagePage";
import type { MessageView } from "../generated/MessageView";
import type { MockState } from "./MockState";

const cursorOf = (m: { journalDate: string; createdAt: number; id: string }): FeedCursor => ({
  journalDate: m.journalDate,
  createdAt: m.createdAt,
  id: m.id,
});

/** Entries in `MockApi`: feed pages (around an entry, newer, older), editing, task boxes, flags, the entry day and moving to another chat. */
export class MockEntries {
  constructor(private readonly s: MockState) {}

  /** Not through `run`: the feed does not keep a call log (slice 3 command checks read it). */
  listMessages(chatId: string, options: ListMessagesOptions = {}): Promise<MessagePage> {
    try {
      return Promise.resolve(this.page(chatId, options));
    } catch (error) {
      return Promise.reject(error);
    }
  }

  editMessage(id: string, body: string): Promise<MessageView> {
    return this.s.run("editMessage", `${id} ${body}`, () => {
      const row = this.s.row(id);
      this.s.writable(row);
      const text = body.replace(/\r\n?/g, "\n").trim();
      if (text === "") throw new KinigeApiError("empty_message");
      return this.s.setBody(row, text);
    });
  }

  toggleTaskItem(id: string, index: number, checked: boolean): Promise<MessageView> {
    return this.s.run("toggleTaskItem", `${id} ${index} ${checked}`, () => {
      const row = this.s.row(id);
      this.s.writable(row);
      return this.s.setBody(row, toggleTaskItem(row.view.bodyMd, index, checked));
    });
  }

  setMessageFlag(id: string, flag: MessageFlag, on: boolean): Promise<MessageView> {
    return this.s.run("setMessageFlag", `${id} ${flag} ${on}`, () => {
      const row = this.s.row(id);
      this.s.writable(row);
      const now = this.s.now().getTime();
      const view = { ...row.view };
      if (flag === "task") {
        view.isTask = on;
        if (!on) view.taskDoneAt = null;
      } else if (flag === "task_done") {
        if (!view.isTask) {
          if (on) throw new KinigeApiError("internal");
        } else view.taskDoneAt = on ? now : null;
      } else {
        view.favoriteAt = on ? now : null;
      }
      row.view = view;
      return view;
    });
  }

  setMessageDate(id: string, journalDate: string): Promise<MessageView> {
    return this.s.run("setMessageDate", `${id} ${journalDate}`, () => {
      const row = this.s.row(id);
      this.s.writable(row);
      if (!isRealDate(journalDate)) throw new KinigeApiError("invalid_date");
      row.view = { ...row.view, journalDate };
      return row.view;
    });
  }

  moveMessage(id: string, chatId: string): Promise<MessageView> {
    return this.s.run("moveMessage", `${id} ${chatId}`, () => {
      const row = this.s.row(id);
      this.s.writable(row);
      if (row.deletedAt !== null) throw new KinigeApiError("message_in_trash");
      if (!this.s.chats.some((chat) => chat.id === chatId)) throw new KinigeApiError("chat_not_found");
      row.view = { ...row.view, chatId };
      return row.view;
    });
  }

  private page(chatId: string, options: ListMessagesOptions): MessagePage {
    const tag = options.tag?.toLowerCase();
    const all = this.s.rows
      .filter((r) => r.view.chatId === chatId && r.deletedAt === null && (tag === undefined || r.view.tags.includes(tag)))
      .map((r) => this.s.viewOf(r))
      .sort(compareFeed);
    const limit = pageLimit(options.limit);
    let start: number;
    let end: number;
    if (options.around !== undefined) {
      const index = all.findIndex((m) => m.id === options.around);
      if (index < 0) throw new KinigeApiError("message_not_found");
      const before = Math.floor((limit - 1) / 2);
      start = Math.max(0, index - before);
      end = Math.min(all.length, index + (limit - 1 - before) + 1);
    } else if (options.after) {
      const after = options.after;
      start = all.findIndex((m) => compareFeed(m, after) > 0);
      if (start < 0) start = all.length;
      end = Math.min(all.length, start + limit);
    } else {
      end = options.before ? all.filter((m) => compareFeed(m, options.before as FeedCursor) < 0).length : all.length;
      start = Math.max(0, end - limit);
    }
    const page = all.slice(start, end);
    const first = page[0];
    const last = page.at(-1);
    return {
      messages: page,
      older: start > 0 && first ? cursorOf(first) : null,
      newer: end < all.length && last ? cursorOf(last) : null,
      today: this.s.journalDate(this.s.now()),
    };
  }
}
