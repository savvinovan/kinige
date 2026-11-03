import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { MockApi, mockMessage } from "../../api/MockApi";
import { I18n } from "../../i18n/I18n";
import { act, renderHook, waitFor } from "../../test/render";
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

    // The reply with the page around an entry: `useFeed` subscribed to it before the test, so by the end of `await reply`
    // the state is already updated, while the render and React's effect cleanup (a separate task) are still ahead.
    release();
    await reply;
    await result.current.reload();
    expect(list).toHaveBeenCalledTimes(1);

    await act(async () => void (await Promise.resolve()));
    expect(result.current.paging.hasNewer).toBe(true);
    expect(result.current.messages.some((m) => m.id === "m100")).toBe(true);
  });
});

describe("useFeed: переход к записи, которой нет", () => {
  it("свежеоткрытый чат: ошибка уходит в onError, а не в loadError; лента показывает последнюю страницу", async () => {
    const api = new MockApi({ now: NOW, messages: MESSAGES });
    const onError = vi.fn();
    const focus = { chatId: INBOX_ID, messageId: "нет-такой", terms: [], nonce: 1 };
    const { result } = renderHook(
      () => useFeed({ api, chatId: INBOX_ID, tag: null, refreshChats: async () => undefined, onError, onTag: vi.fn(), focus }),
      { wrapper },
    );
    await waitFor(() => expect(result.current.loaded).toBe(true));
    expect(onError).toHaveBeenCalledWith("message_not_found");
    expect(result.current.loadError).toBeNull();
    expect(result.current.messages.at(-1)?.id).toBe("m449");
    expect(result.current.paging.hasNewer).toBe(false);
    expect(result.current.scrollTo).toBeNull();
  });
});

describe("useFeed: appendSent", () => {
  it("запись из другого чата (человек успел переключиться, пока шла отправка) в ленту не попадает", async () => {
    const api = new MockApi({ now: NOW, messages: MESSAGES.slice(-3) });
    const other = await api.createChat("Идеи", null);
    const sent = await api.sendMessage(other.id, "запись в идеях");
    const { result } = renderHook(
      () => useFeed({ api, chatId: INBOX_ID, tag: null, refreshChats: async () => undefined, onError: vi.fn(), onTag: vi.fn() }),
      { wrapper },
    );
    await waitFor(() => expect(result.current.loaded).toBe(true));
    act(() => result.current.appendSent(sent));
    expect(result.current.messages.map((m) => m.id)).not.toContain(sent.id);
    expect(result.current.messages).toHaveLength(3);
  });

  it("лента в середине истории: отправленная запись открывает живой край, а не встаёт после старых записей", async () => {
    const api = new MockApi({ now: NOW, messages: MESSAGES });
    const focus = { chatId: INBOX_ID, messageId: "m100", terms: [], nonce: 1 };
    const { result } = renderHook(
      () => useFeed({ api, chatId: INBOX_ID, tag: null, refreshChats: async () => undefined, onError: vi.fn(), onTag: vi.fn(), focus }),
      { wrapper },
    );
    await waitFor(() => expect(result.current.paging.hasNewer).toBe(true));
    const sent = await api.sendMessage(INBOX_ID, "новая запись");
    act(() => result.current.appendSent(sent));
    await waitFor(() => expect(result.current.paging.hasNewer).toBe(false));
    expect(result.current.messages.at(-1)?.id).toBe(sent.id);
    expect(result.current.messages.at(-2)?.id).toBe("m449");
    expect(result.current.scrollTo?.id).toBe(sent.id);
  });
});

const wrapper = ({ children }: { children: ReactNode }) => <I18n language="ru">{children}</I18n>;
