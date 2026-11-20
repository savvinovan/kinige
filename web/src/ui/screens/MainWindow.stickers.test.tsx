import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import { installEmojiLayoutStub } from "../../test/emoji";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { DemoMedia, demoPhoto } from "../attachments/demo";
import { MainWindow } from "./MainWindow";

// Slice 5c on the real `MainWindow` and `MockApi`: the three tabs of the emoji window, a sticker sent with a click (text draft kept, pending reply
// used), the add-pack and remove dialogs, a refused send, a GIF sent again, and the picture of a sticker entry drawn as a sticker.
let restoreLayout: () => void;
beforeAll(() => {
  restoreLayout = installEmojiLayoutStub();
});
afterAll(() => restoreLayout());

const NOW = () => new Date(Date.UTC(2026, 10, 20, 12, 0));
const AT = Date.UTC(2026, 10, 20, 5, 0);
const CAT = "\u{1F63A}";
const TOKEN = "123456:ABC-def_1";

async function setup(options: { token?: boolean; messages?: ReturnType<typeof mockMessage>[]; ready?: string; replyTo?: string } = {}) {
  const api = new MockApi({
    now: NOW,
    language: "en",
    messages: options.messages ?? [mockMessage({ id: "m1", bodyMd: "an entry", createdAt: AT, journalDate: "2026-11-20" })],
  });
  api.stickerLibrary.seedPack("From Telegram", 4, "telegram_export");
  if (options.token) await api.setTelegramBotToken(TOKEN);
  // The "Reply" starts from the "On this day" card in the app; here the draft already carries it, as after a restart.
  if (options.replyTo) await api.saveDraft(INBOX_ID, "", options.replyTo);
  render(
    <DemoMedia>
      <MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />
    </DemoMedia>,
    { language: "en" },
  );
  await screen.findAllByText(options.ready ?? "an entry");
  return api;
}

const emojiButton = () => screen.getByRole("button", { name: "Emoji" });
const panel = () => screen.getByRole("dialog", { name: "Emoji picker" });
const field = () => screen.getByRole("textbox", { name: "New note" }) as HTMLTextAreaElement;
const openStickers = async () => {
  fireEvent.click(emojiButton());
  fireEvent.click(within(panel()).getByRole("tab", { name: "Stickers" }));
  await within(panel()).findByRole("toolbar", { name: "Sticker packs" });
};

describe("the emoji window in the main window", () => {
  it("has the three tabs; a sticker is sent at once into the feed as a sticker, the text in the field stays, the panel stays open", async () => {
    const api = await setup();
    fireEvent.change(field(), { target: { value: "half a thought" } });
    await openStickers();
    fireEvent.click(within(panel()).getAllByRole("button", { name: `Sticker ${CAT}` })[0] as HTMLElement);
    await waitFor(() => expect(document.querySelector(".message--sticker")).not.toBeNull());
    expect(field().value).toBe("half a thought");
    expect(panel()).toBeInTheDocument();
    expect(api.calls.some((call) => call.startsWith("sendSticker"))).toBe(true);
    // Nothing else was sent: the typed text is still a draft.
    expect(api.calls.some((call) => call.startsWith("sendMessage"))).toBe(false);
    // No toast, no status line: the entry simply appears.
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("a pending reply goes with the sticker and the strip above the field is cleared", async () => {
    const api = await setup({ replyTo: "m1" });
    await waitFor(() => expect(document.querySelector(".reply-strip")).not.toBeNull());
    await openStickers();
    fireEvent.click(within(panel()).getAllByRole("button", { name: `Sticker ${CAT}` })[0] as HTMLElement);
    await waitFor(() => expect(document.querySelector(".message--sticker .reply-quote")).not.toBeNull());
    expect(api.calls.some((call) => /^sendSticker \S+ \S+ m1$/.test(call))).toBe(true);
    await waitFor(() => expect(document.querySelector(".reply-strip")).toBeNull());
  });

  it("a refused send is a line above the field, the panel closes so it is seen, nothing is in the feed", async () => {
    const api = await setup();
    await openStickers();
    api.failNext("sendSticker", "sticker_unreadable");
    fireEvent.click(within(panel()).getAllByRole("button", { name: `Sticker ${CAT}` })[0] as HTMLElement);
    expect(await screen.findByRole("alert")).toHaveTextContent("The file of that sticker could not be read. Nothing was sent.");
    expect(screen.queryByRole("dialog", { name: "Emoji picker" })).toBeNull();
    expect(document.querySelector(".message--sticker")).toBeNull();
  });

  it("the + opens the add-pack dialog: no token says so with Open Settings; with a token a pack is added and the next opening of the panel shows it", async () => {
    const api = await setup();
    await openStickers();
    fireEvent.click(within(panel()).getByRole("button", { name: "Add a sticker pack" }));
    expect(screen.queryByRole("dialog", { name: "Emoji picker" })).toBeNull();
    const dialog = await screen.findByRole("dialog", { name: "Add a sticker pack" });
    expect(within(dialog).getByText("Add your bot token in Settings → Stickers")).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Open Settings" }));
    expect(api.openSettingsCount).toBe(1);
    await api.setTelegramBotToken(TOKEN);
    // Back from Settings: the window gets focus again (sent until the dialog, which listens for it, has taken it up).
    await waitFor(() => {
      window.dispatchEvent(new Event("focus"));
      expect(within(dialog).getByRole("textbox", { name: "Pack link" })).toBeInTheDocument();
    });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Pack link" }), { target: { value: "https://t.me/addstickers/TestPack" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add" }));
    await within(dialog).findByText(/Added «Test Pack»: 27 stickers\. 3 animated or video stickers were skipped\./);
    fireEvent.click(within(dialog).getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add a sticker pack" })).toBeNull());
    await openStickers();
    expect(within(panel()).getByRole("button", { name: "Test Pack" })).toBeInTheDocument();
  });

  it("right click on a pack icon, Remove pack, Remove: the pack is gone, the stickers sent from it stay in the feed", async () => {
    const api = await setup();
    await openStickers();
    fireEvent.click(within(panel()).getAllByRole("button", { name: `Sticker ${CAT}` })[0] as HTMLElement);
    await waitFor(() => expect(document.querySelector(".message--sticker")).not.toBeNull());
    fireEvent.contextMenu(within(panel()).getByRole("button", { name: "From Telegram" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Remove pack" }));
    const question = await screen.findByRole("dialog", { name: "Remove pack" });
    expect(within(question).getByText("Remove «From Telegram»? The stickers you sent stay in your entries.")).toBeInTheDocument();
    fireEvent.click(within(question).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Remove pack" })).toBeNull());
    expect((await api.getStickerLibrary()).packs).toEqual([]);
    expect(document.querySelector(".message--sticker")).not.toBeNull();
  });

  it("the GIFs tab sends a GIF of the journal again as a new entry; 'Choose a GIF file…' runs the paperclip flow", async () => {
    const gif = demoPhoto("g1", "cat.gif", 480, 270, { mime: "image/gif", byteSize: 400_000 });
    const old = mockMessage({ id: "m0", bodyMd: "", createdAt: AT - 1000, journalDate: "2026-11-20", attachments: [gif] });
    const api = await setup({ messages: [old, mockMessage({ id: "m1", bodyMd: "an entry", createdAt: AT, journalDate: "2026-11-20" })] });
    fireEvent.click(emojiButton());
    fireEvent.click(within(panel()).getByRole("tab", { name: "GIFs" }));
    fireEvent.click(await within(panel()).findByRole("button", { name: "GIF, 480 by 270" }));
    await waitFor(() => expect(api.calls.some((call) => call.startsWith("sendGifAgain"))).toBe(true));
    expect(panel()).toBeInTheDocument();
    fireEvent.click(within(panel()).getByRole("button", { name: "Choose a GIF file…" }));
    await waitFor(() => expect(api.calls).toContain(`attachPick ${"00000000-0000-4000-8000-000000000001"}`));
    expect(screen.queryByRole("dialog", { name: "Emoji picker" })).toBeNull();
  });
});

describe("an entry from the export", () => {
  it("a sticker entry is drawn as a sticker with its emoji; an entry that was edited after the import shows the picture and keeps its text", async () => {
    const pure = mockMessage({ id: "s1", bodyMd: "", createdAt: AT, journalDate: "2026-11-20", attachments: [demoPhoto("a1", "sticker.webp", 512, 512, { sticker: { emoji: CAT } })] });
    const edited = mockMessage({ id: "s2", bodyMd: "my note", createdAt: AT + 1000, journalDate: "2026-11-20", attachments: [demoPhoto("a2", "sticker.webp", 512, 512, { sticker: { emoji: CAT } })] });
    await setup({ messages: [pure, edited], ready: "my note" });
    const articles = [...document.querySelectorAll("article")];
    expect(articles.map((a) => a.classList.contains("message--sticker"))).toEqual([true, false]);
    expect(articles[1]?.querySelector(".album")).not.toBeNull();
  });
});
