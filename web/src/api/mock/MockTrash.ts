import { KinigeApiError } from "../KinigeApi";
import { resolveColor } from "../mockColor";
import { daysLeft } from "../mockRules";
import type { TrashItem } from "../generated/TrashItem";
import type { MockState } from "./MockState";

/** The trash in `MockApi`: deleting, restoring, the list with the days left, and deleting for good. */
export class MockTrash {
  constructor(private readonly s: MockState) {}

  deleteMessage(id: string): Promise<void> {
    return this.s.run("deleteMessage", id, () => {
      const row = this.s.row(id);
      row.deletedAt ??= this.s.now().getTime();
    });
  }

  restoreMessage(id: string): Promise<void> {
    return this.s.run("restoreMessage", id, () => {
      this.s.row(id).deletedAt = null;
    });
  }

  listTrash(): Promise<TrashItem[]> {
    const now = this.s.now().getTime();
    return this.s.run("listTrash", "", () =>
      this.s.rows
        .filter((r) => r.deletedAt !== null)
        .sort((a, b) => (b.deletedAt ?? 0) - (a.deletedAt ?? 0))
        .map((r) => {
          const chat = this.s.chats.find((c) => c.id === r.view.chatId);
          const deletedAt = r.deletedAt ?? 0;
          return {
            message: r.view,
            chatTitle: chat?.title ?? "",
            chatIcon: chat?.icon ?? null,
            chatIsInbox: chat?.isInbox ?? false,
            chatColor: chat?.color ?? resolveColor(r.view.chatId, null),
            deletedAt,
            daysLeft: daysLeft(deletedAt, now),
          };
        }),
    );
  }

  purgeMessage(id: string): Promise<void> {
    return this.s.run("purgeMessage", id, () => {
      const row = this.s.row(id);
      if (row.deletedAt === null) throw new KinigeApiError("not_in_trash");
      this.s.rows = this.s.rows.filter((r) => r !== row);
    });
  }
}
