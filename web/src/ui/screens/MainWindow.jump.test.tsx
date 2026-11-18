import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import { FakeObserver, edges, showEdge } from "../../test/fakeObserver";
import { configure, fireEvent, render, screen, waitFor } from "../../test/render";
import { MainWindow } from "./MainWindow";

// "Jump to latest" in the window (KINIG-145): a feed opened on an old entry holds only a window around it; the click loads the latest page
// instead of paging through the history in between, and the composer takes the focus, as after sending.
vi.setConfig({ testTimeout: 30_000 });
configure({ asyncUtilTimeout: 20_000 });

const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const HOUR = 3_600_000;
const START = Date.UTC(2024, 0, 1, 12);
const journalDate = (at: number) => new Date(at - 4 * HOUR).toISOString().slice(0, 10);
const MANY = Array.from({ length: 1000 }, (_, i) => {
  const createdAt = START + i * HOUR;
  return mockMessage({ id: `m${i}`, bodyMd: `запись ${i} слово${i}`, createdAt, journalDate: journalDate(createdAt) });
});

beforeEach(() => {
  FakeObserver.all = [];
  vi.stubGlobal("IntersectionObserver", FakeObserver);
});
afterEach(() => vi.unstubAllGlobals());

const plateOf = (container: HTMLElement, id: string) => container.querySelector<HTMLElement>(`[data-message-id="${id}"] article`);
const jumpButton = () => screen.queryByRole("button", { name: "К последним записям" });

async function openOldEntry() {
  const api = new MockApi({ now: NOW, messages: MANY });
  const view = render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
  await screen.findByText("запись 999 слово999");
  fireEvent.change(screen.getByRole("combobox", { name: "Поиск" }), { target: { value: "слово300" } });
  fireEvent.click(await screen.findByRole("option", { name: /слово300/ }));
  await waitFor(() => expect(plateOf(view.container, "m300")).not.toBeNull());
  return { api, ...view };
}

describe("Лента: «К последним записям»", () => {
  it("открыта старая запись: кнопка есть; клик загружает последнюю страницу, а не листает историю; фокус в поле ввода", async () => {
    const { api, container } = await openOldEntry();
    expect(plateOf(container, "m999")).toBeNull();
    expect(edges(container)).toHaveLength(2);
    const calls = vi.spyOn(api, "listMessages");
    fireEvent.click(jumpButton() as HTMLElement);
    await waitFor(() => expect(plateOf(container, "m999")).not.toBeNull());
    // One request, for the last page; the pages between the old entry and the end are not loaded one by one.
    expect(calls).toHaveBeenCalledTimes(1);
    expect(calls.mock.calls[0]?.[1]).toEqual({ limit: 200 });
    expect(plateOf(container, "m300")).toBeNull();
    // The feed is live again: no newer edge, no button, the composer has the focus.
    expect(edges(container)).toHaveLength(1);
    await waitFor(() => expect(jumpButton()).toBeNull());
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Новая заметка" })).toHaveFocus());
  });

  it("страница снизу, запрошенная до клика и пришедшая после него, не возвращает ленту в середину истории", async () => {
    const { api, container } = await openOldEntry();
    const original = api.listMessages.bind(api);
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    vi.spyOn(api, "listMessages").mockImplementation(async (chatId, options) => {
      if (options?.after !== undefined) await gate;
      return original(chatId, options);
    });
    // The newer edge is reached: the page below is requested and held.
    await showEdge(container, "newer");
    fireEvent.click(jumpButton() as HTMLElement);
    await waitFor(() => expect(plateOf(container, "m999")).not.toBeNull());
    release();
    await act0();
    expect(edges(container)).toHaveLength(1);
    expect(jumpButton()).toBeNull();
    expect(plateOf(container, "m300")).toBeNull();
  });

  it("лента у конца истории: кнопки нет, пока не отошли вверх больше чем на экран", async () => {
    const api = new MockApi({ now: NOW, messages: MANY });
    const { container } = render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
    await screen.findByText("запись 999 слово999");
    expect(jumpButton()).toBeNull();
    const scroller = container.querySelector(".feed-scroll") as HTMLElement;
    Object.defineProperty(scroller, "clientHeight", { configurable: true, value: 500 });
    scroller.scrollTop = -2000;
    fireEvent.scroll(scroller);
    fireEvent.click(jumpButton() as HTMLElement);
    await waitFor(() => expect(scroller.scrollTop).toBe(0));
    await waitFor(() => expect(jumpButton()).toBeNull());
    expect(plateOf(container, "m999")).not.toBeNull();
  });

  it("в корзине кнопки нет", async () => {
    const api = new MockApi({ now: NOW, messages: MANY });
    render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
    await screen.findByText("запись 999 слово999");
    fireEvent.click(screen.getByRole("button", { name: /Корзина/ }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Записи" })).toBeNull());
    expect(jumpButton()).toBeNull();
  });
});

/** Lets the pending promises of the held request settle. */
const act0 = () => new Promise((resolve) => setTimeout(resolve, 50));
