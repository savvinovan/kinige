import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { demoPhoto } from "../attachments/demo";
import { installEmojiLayoutStub } from "../../test/emoji";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import type { PickerTab } from "./PickerTabs";
import { type Panel, makeHost, panelElement, seededApi } from "./stickerTestKit";

let restoreLayout: () => void;
beforeAll(() => {
  restoreLayout = installEmojiLayoutStub();
});
afterAll(() => restoreLayout());

const CAT = "\u{1F63A}";

function open(replyId: string | null = null) {
  const api = seededApi();
  const panel: Panel = { api, onClose: vi.fn(), onPick: vi.fn(), host: makeHost(api, replyId) };
  render(panelElement(panel), { language: "en" });
  return panel;
}

const dialog = () => screen.getByRole("dialog", { name: "Emoji picker" });
const tab = (name: string) => within(dialog()).getByRole("tab", { name });
const showStickers = async () => {
  fireEvent.click(tab("Stickers"));
  await screen.findByRole("toolbar", { name: "Sticker packs" });
};
const stickerButtons = () => within(dialog()).getAllByRole("button", { name: /^Sticker/ });
const calls = (panel: Panel) => panel.api.calls.join("\n");

describe("the emoji window with tabs", () => {
  it("opens on Emoji with focus in the search, as before; three tabs, the selected one is marked and in the Tab order", () => {
    open();
    const tabs = within(dialog()).getAllByRole("tab");
    expect(tabs.map((t) => t.textContent)).toEqual(["Emoji", "Stickers", "GIFs"]);
    expect(within(dialog()).getByRole("tablist", { name: "Emoji, stickers and GIFs" })).toBeInTheDocument();
    expect(tab("Emoji")).toHaveAttribute("aria-selected", "true");
    expect(tabs.map((t) => t.tabIndex)).toEqual([0, -1, -1]);
    expect(within(dialog()).getByRole("searchbox", { name: "Search emoji" })).toHaveFocus();
    // The open panel is labelled by its tab.
    expect(within(dialog()).getByRole("tabpanel")).toHaveAccessibleName("Emoji");
  });

  it("opens on the tab it was last left on, for the session only: the host's in-memory ref, no setting", async () => {
    const api = seededApi();
    const lastTab: { current: PickerTab } = { current: "emoji" };
    const tabMemory = { recall: () => lastTab.current, remember: (next: PickerTab) => void (lastTab.current = next) };
    const panel: Panel = { api, onClose: vi.fn(), onPick: vi.fn(), host: { ...makeHost(api), tabMemory } };
    const first = render(panelElement(panel), { language: "en" });
    expect(tab("Emoji")).toHaveAttribute("aria-selected", "true");
    await showStickers();
    expect(lastTab.current).toBe("stickers");
    first.unmount();
    // The next opening (a new popover on the same host) is on Stickers, and reads the library, not the emoji data.
    render(panelElement(panel), { language: "en" });
    expect(tab("Stickers")).toHaveAttribute("aria-selected", "true");
    await screen.findByRole("toolbar", { name: "Sticker packs" });
    // Focus is inside the dialog, on the selected tab (the emoji search is not there to take it): Tab walks the panel and Esc closes it, from there.
    expect(tab("Stickers")).toHaveFocus();
    fireEvent.keyDown(document.activeElement as Element, { key: "Tab" });
    expect(dialog().contains(document.activeElement)).toBe(true);
    expect(document.activeElement).not.toBe(tab("Stickers"));
    expect(panel.onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(document.activeElement as Element, { key: "Escape" });
    expect(panel.onClose).toHaveBeenCalledWith(true);
    fireEvent.click(tab("Emoji"));
    expect(lastTab.current).toBe("emoji");
    expect(Object.keys(window.localStorage ?? {})).toEqual([]);
    expect(calls(panel)).not.toMatch(/setSettings|updateSettings/);
  });

  it("nothing is read before a tab is shown: not at opening, not while only Emoji is open", async () => {
    const panel = open();
    await act(async () => undefined);
    expect(calls(panel)).not.toMatch(/getStickerLibrary|listJournalGifs/);
    fireEvent.click(tab("Stickers"));
    await screen.findByRole("toolbar", { name: "Sticker packs" });
    expect(calls(panel)).toContain("getStickerLibrary");
    expect(calls(panel)).not.toContain("listJournalGifs");
    fireEvent.click(tab("GIFs"));
    await screen.findByText("No GIFs yet: drop, paste or attach a GIF");
    expect(calls(panel)).toContain("listJournalGifs");
  });

  it("the arrows on the tab list move to the next tab, select it and leave focus on the tab list; Home and End go to the edges", async () => {
    open();
    tab("Emoji").focus();
    fireEvent.keyDown(tab("Emoji"), { key: "ArrowRight" });
    expect(tab("Stickers")).toHaveAttribute("aria-selected", "true");
    expect(tab("Stickers")).toHaveFocus();
    await screen.findByRole("toolbar", { name: "Sticker packs" });
    fireEvent.keyDown(tab("Stickers"), { key: "End" });
    expect(tab("GIFs")).toHaveFocus();
    fireEvent.keyDown(tab("GIFs"), { key: "ArrowRight" });
    expect(tab("Emoji")).toHaveFocus();
    // Back on Emoji by the keyboard: focus stays on the tab, it is not pulled into the search.
    expect(within(dialog()).getByRole("searchbox", { name: "Search emoji" })).not.toHaveFocus();
    fireEvent.keyDown(tab("Emoji"), { key: "ArrowLeft" });
    expect(tab("GIFs")).toHaveFocus();
  });

  it("Esc closes with focus returned; Tab stays inside the panel and goes round it, Shift+Tab back", async () => {
    const panel = open();
    await showStickers();
    tab("Stickers").focus();
    fireEvent.keyDown(tab("Stickers"), { key: "Tab" });
    // The next control after the tab list is the strip, then the grid, and after the last one the tab list again.
    const first = within(dialog()).getAllByRole("button")[0];
    expect(dialog().contains(document.activeElement)).toBe(true);
    expect(first).toBeDefined();
    fireEvent.keyDown(document.activeElement ?? dialog(), { key: "Tab", shiftKey: true });
    expect(tab("Stickers")).toHaveFocus();
    expect(panel.onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(tab("Stickers"), { key: "Escape" });
    expect(panel.onClose).toHaveBeenCalledExactlyOnceWith(true);
  });
});

describe("the Stickers tab", () => {
  it("a strip of pack icons (Recent only once something was sent, From Telegram, the packs, +) above a grid under pack headings", async () => {
    const panel = open();
    await showStickers();
    const strip = within(dialog()).getByRole("toolbar", { name: "Sticker packs" });
    expect(within(strip).getAllByRole("button").map((b) => b.getAttribute("aria-label"))).toEqual(["From Telegram", "Cats", "Add a sticker pack"]);
    expect(within(dialog()).getAllByRole("heading").map((h) => h.textContent)).toEqual(["From Telegram", "Cats"]);
    expect(stickerButtons()).toHaveLength(9);
    expect(panel.api.calls.some((c) => c.startsWith("sendSticker"))).toBe(false);
  });

  it("every sticker is a button labelled 'Sticker <emoji>'", async () => {
    open();
    await showStickers();
    const first = stickerButtons()[0];
    expect(first).toHaveAccessibleName(`Sticker ${CAT}`);
    expect(first?.tagName).toBe("BUTTON");
    expect(first).toHaveAttribute("type", "button");
  });

  it("a click sends it at once to the open chat with the pending reply; the panel stays open and the text draft is untouched", async () => {
    const api = seededApi();
    await api.saveDraft(INBOX_ID, "half a thought");
    const target = await api.sendMessage(INBOX_ID, "the entry I reply to");
    await api.saveDraft(INBOX_ID, "half a thought", target.id);
    const panel: Panel = { api, onClose: vi.fn(), onPick: vi.fn(), host: makeHost(api, target.id) };
    render(panelElement(panel), { language: "en" });
    await showStickers();
    const [first] = stickerButtons();
    if (!first) throw new Error("no sticker");
    fireEvent.click(first);
    await waitFor(() => expect(calls(panel)).toContain(`sendSticker ${INBOX_ID}`));
    const sent = (await api.listMessages(INBOX_ID)).messages.find((message) => message.attachments.some((a) => a.sticker));
    expect(sent?.bodyMd).toBe("");
    expect(sent?.replyTo?.id).toBe(target.id);
    expect(sent?.attachments[0]?.stickerEmoji).toBe(CAT);
    expect((await api.getDraft(INBOX_ID)).body).toBe("half a thought");
    expect(panel.onClose).not.toHaveBeenCalled();
    expect(panel.onPick).not.toHaveBeenCalled();
  });

  it("the grid keyboard: one Tab stop, the arrows move by cell and by row, Home and End to the edges, and a sticker is a native button (Enter presses it)", async () => {
    const panel = open();
    await showStickers();
    const stickers = stickerButtons();
    expect(stickers.filter((s) => s.tabIndex === 0)).toEqual([stickers[0]]);
    stickers[0]?.focus();
    fireEvent.keyDown(stickers[0] as HTMLElement, { key: "ArrowRight" });
    expect(stickers[1]).toHaveFocus();
    fireEvent.keyDown(stickers[1] as HTMLElement, { key: "ArrowDown" });
    // The first pack has 6 stickers: row two has one cell (index 5); down from column 1 goes to the nearest cell of that row.
    expect(stickers[5]).toHaveFocus();
    fireEvent.keyDown(stickers[5] as HTMLElement, { key: "ArrowDown" });
    // Into the next pack, first row, column 0.
    expect(stickers[6]).toHaveFocus();
    fireEvent.keyDown(stickers[6] as HTMLElement, { key: "ArrowUp" });
    expect(stickers[5]).toHaveFocus();
    fireEvent.keyDown(stickers[5] as HTMLElement, { key: "End" });
    expect(stickers[8]).toHaveFocus();
    fireEvent.keyDown(stickers[8] as HTMLElement, { key: "Home" });
    expect(stickers[0]).toHaveFocus();
    expect(stickers.filter((s) => s.tabIndex === 0)).toEqual([stickers[0]]);
    // Nothing was sent by moving.
    expect(calls(panel)).not.toContain("sendSticker");
  });

  it("Recent comes first, the most recently sent first, once something was sent (read again when the tab is opened again)", async () => {
    const api = seededApi();
    const [a, b] = (await api.getStickerLibrary()).packs[0]?.stickers ?? [];
    if (!a || !b) throw new Error("no stickers");
    await api.sendSticker(INBOX_ID, a.id);
    await api.sendSticker(INBOX_ID, b.id);
    render(panelElement({ api, onClose: vi.fn(), onPick: vi.fn(), host: makeHost(api) }), { language: "en" });
    await showStickers();
    expect(within(dialog()).getAllByRole("heading").map((h) => h.textContent)).toEqual(["Recent", "From Telegram", "Cats"]);
    const strip = within(dialog()).getByRole("toolbar", { name: "Sticker packs" });
    expect(within(strip).getAllByRole("button")[0]).toHaveAccessibleName("Recent");
    const recent = within(dialog().querySelector("section") as HTMLElement).getAllByRole("button");
    expect(recent).toHaveLength(2);
  });

  it("the strip marks the pack whose section is at the top of the grid: on opening, after a click on an icon, and while the grid scrolls", async () => {
    open();
    await showStickers();
    const strip = within(dialog()).getByRole("toolbar", { name: "Sticker packs" });
    const icon = (name: string) => within(strip).getByRole("button", { name });
    const marked = () => within(strip).getAllByRole("button").filter((b) => b.getAttribute("aria-current") === "true").map((b) => b.getAttribute("aria-label"));
    expect(marked()).toEqual(["From Telegram"]);
    fireEvent.click(icon("Cats"));
    expect(marked()).toEqual(["Cats"]);
    // The grid scrolls: the sections report where they are (jsdom has no layout), and the mark follows the last one that reached the top.
    const scroll = dialog().querySelector(".sticker-panel__scroll") as HTMLElement;
    const sections = [...dialog().querySelectorAll<HTMLElement>(".sticker-panel__section")];
    const at = (y: number) => () => ({ top: y }) as DOMRect;
    scroll.getBoundingClientRect = at(100);
    sections[0]!.getBoundingClientRect = at(20);
    sections[1]!.getBoundingClientRect = at(300);
    fireEvent.scroll(scroll);
    expect(marked()).toEqual(["From Telegram"]);
    sections[1]!.getBoundingClientRect = at(104);
    fireEvent.scroll(scroll);
    expect(marked()).toEqual(["Cats"]);
    // A last pack shorter than the grid never reaches its top: at the end of the scroll it is the marked one (the metrics are the area's own).
    sections[1]!.getBoundingClientRect = at(300);
    const metrics = (height: number, client: number, top: number) => {
      for (const [name, value] of Object.entries({ scrollHeight: height, clientHeight: client, scrollTop: top })) Object.defineProperty(scroll, name, { value, configurable: true });
    };
    metrics(1000, 300, 400);
    fireEvent.scroll(scroll);
    expect(marked()).toEqual(["From Telegram"]);
    metrics(1000, 300, 700);
    fireEvent.scroll(scroll);
    expect(marked()).toEqual(["Cats"]);
    // Nothing to scroll (no layout, or everything fits): the end rule does not apply.
    metrics(300, 300, 0);
    fireEvent.scroll(scroll);
    expect(marked()).toEqual(["From Telegram"]);
    // The "+" is never marked.
    expect(icon("Add a sticker pack")).not.toHaveAttribute("aria-current");
  });

  it("empty: one line tells how to get stickers, and the strip with the + stays", async () => {
    const api = seededApi();
    for (const pack of (await api.getStickerLibrary()).packs) await api.removeStickerPack(pack.id);
    render(panelElement({ api, onClose: vi.fn(), onPick: vi.fn(), host: makeHost(api) }), { language: "en" });
    await showStickers();
    expect(screen.getByText("No stickers yet: import your Telegram export or add a pack by its link")).toBeInTheDocument();
    expect(within(dialog()).getByRole("button", { name: "Add a sticker pack" })).toBeInTheDocument();
  });

  it("the + closes the panel (focus back to the field) and asks the window for the add-pack dialog", async () => {
    const panel = open();
    await showStickers();
    fireEvent.click(within(dialog()).getByRole("button", { name: "Add a sticker pack" }));
    expect(panel.onClose).toHaveBeenCalledWith(true);
    expect(panel.host.onAddPack).toHaveBeenCalledOnce();
  });

  it("right click on a pack icon offers 'Remove pack' (and a click on that menu is not an outside click); Recent has no menu", async () => {
    const panel = open();
    await showStickers();
    const cats = within(dialog()).getByRole("button", { name: "Cats" });
    fireEvent.contextMenu(cats);
    const item = screen.getByRole("menuitem", { name: "Remove pack" });
    fireEvent.pointerDown(item);
    expect(panel.onClose).not.toHaveBeenCalled();
    fireEvent.click(item);
    expect(panel.host.onRemovePack).toHaveBeenCalledWith(expect.objectContaining({ title: "Cats" }));
    expect(panel.onClose).toHaveBeenCalledWith(true);
  });

  it("a failed send closes the panel so the error above the field is seen", async () => {
    const api = seededApi();
    const host = makeHost(api);
    host.sendSticker = () => Promise.reject(new Error("refused"));
    const panel: Panel = { api, onClose: vi.fn(), onPick: vi.fn(), host };
    render(panelElement(panel), { language: "en" });
    await showStickers();
    fireEvent.click(stickerButtons()[0] as HTMLElement);
    await waitFor(() => expect(panel.onClose).toHaveBeenCalledWith(true));
  });

  it("a library that cannot be read says so", async () => {
    const api = seededApi();
    api.failNext("getStickerLibrary", "store_read_failed");
    render(panelElement({ api, onClose: vi.fn(), onPick: vi.fn(), host: makeHost(api) }), { language: "en" });
    fireEvent.click(tab("Stickers"));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not load the stickers.");
  });
});

describe("the GIFs tab", () => {
  const gif = (id: string, name: string) => demoPhoto(id, name, 480, 270, { mime: "image/gif", byteSize: 500_000 });

  it("the journal's GIFs, newest first, as still frames labelled with their size and carrying the badge; a click sends one again", async () => {
    const api = seededApi();
    api.addMessageWithAttachments({ id: "m1", bodyMd: "", journalDate: "2026-11-01", createdAt: Date.UTC(2026, 10, 1) }, [gif("g1", "old.gif")]);
    api.addMessageWithAttachments({ id: "m2", bodyMd: "", journalDate: "2026-11-02", createdAt: Date.UTC(2026, 10, 2) }, [gif("g2", "new.gif")]);
    const panel: Panel = { api, onClose: vi.fn(), onPick: vi.fn(), host: makeHost(api) };
    render(panelElement(panel), { language: "en" });
    fireEvent.click(tab("GIFs"));
    const cells = await screen.findAllByRole("button", { name: "GIF, 480 by 270" });
    expect(cells).toHaveLength(2);
    expect(cells[0]?.querySelector("img")?.getAttribute("src")).toContain("data:image/svg+xml");
    expect(within(cells[0] as HTMLElement).getByText("GIF")).toBeInTheDocument();
    fireEvent.click(cells[0] as HTMLElement);
    await waitFor(() => expect(calls(panel)).toContain(`sendGifAgain ${INBOX_ID} g2`));
    expect(panel.onClose).not.toHaveBeenCalled();
  });

  it("empty: the line says how to get a GIF; 'Choose a GIF file…' closes the panel and runs the paperclip flow", async () => {
    const panel = open();
    fireEvent.click(tab("GIFs"));
    expect(await screen.findByText("No GIFs yet: drop, paste or attach a GIF")).toBeInTheDocument();
    fireEvent.click(within(dialog()).getByRole("button", { name: "Choose a GIF file…" }));
    expect(panel.onClose).toHaveBeenCalledWith(true);
    expect(panel.host.onChooseGif).toHaveBeenCalledOnce();
  });

  it("the arrows move over the three columns", async () => {
    const api = seededApi();
    for (let i = 0; i < 5; i++) {
      api.addMessageWithAttachments({ id: `m${i}`, bodyMd: "", journalDate: "2026-11-02", createdAt: Date.UTC(2026, 10, 2, i) }, [gif(`g${i}`, `${i}.gif`)]);
    }
    render(panelElement({ api, onClose: vi.fn(), onPick: vi.fn(), host: makeHost(api) }), { language: "en" });
    fireEvent.click(tab("GIFs"));
    const cells = await screen.findAllByRole("button", { name: "GIF, 480 by 270" });
    cells[0]?.focus();
    fireEvent.keyDown(cells[0] as HTMLElement, { key: "ArrowDown" });
    expect(cells[3]).toHaveFocus();
    fireEvent.keyDown(cells[3] as HTMLElement, { key: "ArrowRight" });
    expect(cells[4]).toHaveFocus();
  });
});
