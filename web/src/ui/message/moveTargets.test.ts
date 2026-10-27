import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import type { SidebarChat } from "../sidebar/chats";
import { moveTargetsFrom } from "./moveTargets";

const t = ((key: string) => (key === "chat.inbox" ? "Входящие" : key)) as unknown as TFunction;
const chat = (id: string, name: string, extra: Partial<SidebarChat> = {}): SidebarChat => ({ id, name, emoji: "📌", ...extra });

describe("moveTargetsFrom", () => {
  it("порядок сайдбара: закреплённые, обычные, архив; текущий чат отмечен, у Inbox переведённое имя", () => {
    const targets = moveTargetsFrom(
      [chat("a", "А"), chat("inbox", "", { isInbox: true, emoji: "📥" }), chat("z", "Старый", { archived: true }), chat("p", "Пин", { pinned: true })],
      "inbox",
      t,
    );
    expect(targets.map((x) => [x.id, x.group, x.current])).toEqual([
      ["p", "pinned", false],
      ["a", "chats", false],
      ["inbox", "chats", true],
      ["z", "archive", false],
    ]);
    expect(targets.find((x) => x.id === "inbox")?.name).toBe("Входящие");
  });

  it("без открытого чата (корзина) текущего нет", () => {
    expect(moveTargetsFrom([chat("a", "А")], null, t).some((x) => x.current)).toBe(false);
  });
});
