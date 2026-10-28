import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { MockApi, mockMessage } from "../../api/MockApi";
import { I18n } from "../../i18n/I18n";
import { act, renderHook } from "../../test/render";
import { useFeed } from "./useFeed";

const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const HOUR = 3_600_000;
const START = Date.UTC(2024, 0, 1, 12);
const journalDate = (at: number) => new Date(at - 4 * HOUR).toISOString().slice(0, 10);

const MESSAGES = Array.from({ length: 450 }, (_, i) => {
  const createdAt = START + i * HOUR;
  return mockMessage({ id: `m${i}`, bodyMd: `запись ${i}`, createdAt, journalDate: journalDate(createdAt) });
});

describe("useFeed: курсоры страниц", () => {
  it("курсор новой страницы доступен сразу после загрузки: «записи изменились» в середине истории ленту не перечитывает", async () => {
    const api = new MockApi({ now: NOW, messages: MESSAGES });
    const answer = api.listMessages.bind(api);
    let release: () => void = () => undefined;
    const held = new Promise<void>((resolve) => (release = resolve));
    let reply: Promise<unknown> = held;
    const list = vi.spyOn(api, "listMessages").mockImplementationOnce((...args) => {
      const page = held.then(() => answer(...args));
      reply = page;
      return page;
    });
    const focus = { chatId: INBOX_ID, messageId: "m100", terms: [], nonce: 1 };
    const { result } = renderHook(
      () => useFeed({ api, chatId: INBOX_ID, tag: null, refreshChats: async () => undefined, onError: vi.fn(), onTag: vi.fn(), focus }),
      { wrapper },
    );
    expect(list).toHaveBeenCalledTimes(1);

    // Ответ на страницу вокруг записи: `useFeed` подписался на него раньше теста, поэтому к концу `await reply` состояние уже
    // обновлено, а отрисовка и сброс эффектов React (отдельная задача) ещё впереди.
    release();
    await reply;
    await result.current.reload();
    expect(list).toHaveBeenCalledTimes(1);

    await act(async () => void (await Promise.resolve()));
    expect(result.current.paging.hasNewer).toBe(true);
    expect(result.current.messages.some((m) => m.id === "m100")).toBe(true);
  });
});

const wrapper = ({ children }: { children: ReactNode }) => <I18n language="ru">{children}</I18n>;
