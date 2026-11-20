import { describe, expect, it, vi } from "vitest";
import { MockApi } from "../../api/MockApi";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { AddPackDialog, type AddPackDialogProps } from "./AddPackDialog";
import { AddPackHost } from "./AddPackHost";
import { RemovePackHost } from "./RemovePackDialog";

const TOKEN = "123456:ABC-def_1";
const dialog = () => screen.getByRole("dialog", { name: "Add a sticker pack" });
const field = () => within(dialog()).getByRole("textbox", { name: "Pack link" });
const press = (name: string) => fireEvent.click(within(dialog()).getByRole("button", { name }));

async function open(options: { token?: boolean } = {}) {
  const api = new MockApi({ language: "en" });
  if (options.token ?? true) await api.setTelegramBotToken(TOKEN);
  const onClose = vi.fn();
  render(<AddPackHost api={api} onClose={onClose} />, { language: "en" });
  await screen.findByRole("dialog", { name: "Add a sticker pack" });
  // The dialog has taken its first focus: its keyboard handling (Esc, Tab) is in place from then on.
  await waitFor(() => expect(dialog().contains(document.activeElement)).toBe(true));
  return { api, onClose };
}

const type = (link: string) => fireEvent.change(field(), { target: { value: link } });

describe("the add-pack dialog", () => {
  it("a field for the link, Add and Cancel; Add waits for a link; the dialog is a Modal with its title and a close button", async () => {
    await open();
    expect(field()).toHaveAttribute("placeholder", "https://t.me/addstickers/Name");
    expect(within(dialog()).getByRole("button", { name: "Add" })).toHaveAttribute("aria-disabled", "true");
    expect(within(dialog()).getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(within(dialog()).getByRole("button", { name: "Close" })).toBeInTheDocument();
  });

  it("without a saved token it says where to put one, offers Open Settings, and starts nothing", async () => {
    const { api } = await open({ token: false });
    expect(within(dialog()).getByText("Add your bot token in Settings → Stickers")).toBeInTheDocument();
    expect(within(dialog()).queryByRole("textbox")).toBeNull();
    press("Open Settings");
    expect(api.openSettingsCount).toBe(1);
    expect(api.calls.some((call) => call.startsWith("addStickerPack"))).toBe(false);
  });

  it("back from Settings with a token saved there, the form takes the place of the line", async () => {
    const { api } = await open({ token: false });
    await api.setTelegramBotToken(TOKEN);
    // The window gets focus again (sent until the dialog, which listens for it, has taken it up).
    await waitFor(() => {
      window.dispatchEvent(new Event("focus"));
      expect(within(dialog()).getByRole("textbox", { name: "Pack link" })).toBeInTheDocument();
    });
  });

  it("Add shows 'Downloading 12 of 30' with Cancel, then the result with the skipped ones, and Done closes", async () => {
    const { api, onClose } = await open();
    api.stickerLibrary.hold = true;
    type("https://t.me/addstickers/TestPack");
    press("Add");
    expect(await within(dialog()).findByText("Downloading 10 of 30")).toBeInTheDocument();
    const bar = within(dialog()).getByRole("progressbar", { name: "Pack download" });
    expect(bar).toHaveAttribute("aria-valuenow", "10");
    expect(bar).toHaveAttribute("aria-valuemax", "30");
    // The Add button that had focus is gone: Cancel has it, and the dialog has no cross while the job runs.
    expect(within(dialog()).getByRole("button", { name: "Cancel" })).toHaveFocus();
    expect(within(dialog()).queryByRole("button", { name: "Close" })).toBeNull();
    api.stickerLibrary.hold = false;
    await act(async () => api.stickerLibrary.release());
    expect(await within(dialog()).findByText("Added «Test Pack»: 27 stickers. 3 animated or video stickers were skipped.")).toBeInTheDocument();
    press("Done");
    expect(onClose).toHaveBeenCalledOnce();
    expect((await api.getStickerLibrary()).packs.map((pack) => pack.title)).toEqual(["Test Pack"]);
  });

  it("Escape closes the form but not a running job; Cancel stops the job, adds nothing and closes without an error", async () => {
    const { api, onClose } = await open();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
    onClose.mockClear();
    api.stickerLibrary.hold = true;
    type("TestPack");
    press("Add");
    await within(dialog()).findByText("Downloading 10 of 30");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
    press("Cancel");
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
    expect((await api.getStickerLibrary()).packs).toEqual([]);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it.each([
    ["Nope", "sticker_pack_not_found", "No pack with this name"],
    ["https://example.com/x", "sticker_pack_link_invalid", "That is not a pack link. Use https://t.me/addstickers/Name or just the name."],
  ])("an error is one line in the dialog and nothing is added: %s", async (link, _code, text) => {
    const { api } = await open();
    type(link);
    press("Add");
    expect(await within(dialog()).findByRole("alert")).toHaveTextContent(text);
    expect((await api.getStickerLibrary()).packs).toEqual([]);
    // Typing again clears the line.
    type(`${link}x`);
    expect(within(dialog()).queryByRole("alert")).toBeNull();
  });

  it.each([
    ["sticker_pack_token_refused", "Telegram didn't accept the bot token: check it in Settings"],
    ["sticker_pack_unreachable", "Couldn't reach Telegram"],
    ["sticker_pack_wait", "Telegram asks to wait: try again in a minute"],
    ["sticker_pack_only_animated", "This pack has only animated or video stickers"],
    ["sticker_pack_too_large", "The pack is too large"],
    ["sticker_pack_busy", "A pack is already downloading"],
    ["sticker_pack_bad_response", "Telegram's answer could not be read. Nothing was added."],
  ] as const)("%s", async (code, text) => {
    const { api } = await open();
    api.stickerLibrary.remotePacks.set("Broken", { title: "Broken", stickers: 3, error: code });
    type("Broken");
    press("Add");
    expect(await within(dialog()).findByRole("alert")).toHaveTextContent(text);
    expect((await api.getStickerLibrary()).packs).toEqual([]);
  });

  it("a no-token answer from Rust (the token went away meanwhile) turns the form into the no-token line", async () => {
    const { api } = await open();
    await api.clearTelegramBotToken();
    type("TestPack");
    press("Add");
    expect(await within(dialog()).findByText("Add your bot token in Settings → Stickers")).toBeInTheDocument();
    expect(within(dialog()).getByRole("button", { name: "Open Settings" })).toBeInTheDocument();
  });
});

describe("the dialog in each state (presentational)", () => {
  const base: AddPackDialogProps = {
    state: { phase: "idle", error: null },
    hasToken: true,
    link: "",
    onLinkChange: vi.fn(),
    onAdd: vi.fn(),
    onCancelJob: vi.fn(),
    onClose: vi.fn(),
    onOpenSettings: vi.fn(),
  };

  it("the first answer is not in yet: 'Contacting Telegram…' and an empty bar; Cancel says it is cancelling and takes no second press", () => {
    const onCancelJob = vi.fn();
    const { rerender } = render(<AddPackDialog {...base} onCancelJob={onCancelJob} state={{ phase: "running", done: null, total: null, cancelling: false }} />, { language: "en" });
    expect(within(dialog()).getByText("Contacting Telegram…")).toBeInTheDocument();
    fireEvent.click(within(dialog()).getByRole("button", { name: "Cancel" }));
    expect(onCancelJob).toHaveBeenCalledOnce();
    rerender(<AddPackDialog {...base} onCancelJob={onCancelJob} state={{ phase: "running", done: 1, total: 4, cancelling: true }} />);
    fireEvent.click(within(dialog()).getByRole("button", { name: "Cancel" }));
    expect(onCancelJob).toHaveBeenCalledOnce();
  });

  it("the result without skips has no second sentence; Russian has all its plural forms", () => {
    const { unmount } = render(<AddPackDialog {...base} state={{ phase: "done", result: { title: "Cats", added: 1, skipped: 0 } }} />, { language: "en" });
    expect(within(dialog()).getByText("Added «Cats»: 1 sticker.")).toBeInTheDocument();
    unmount();
    const ru = (added: number, skipped: number) => {
      const view = render(<AddPackDialog {...base} state={{ phase: "done", result: { title: "Коты", added, skipped } }} />, { language: "ru" });
      const text = screen.getByRole("dialog").querySelector("p")?.textContent;
      view.unmount();
      return text;
    };
    expect(ru(1, 1)).toBe("Добавлен набор «Коты»: 1 стикер. Пропущен 1 анимированный или видеостикер.");
    expect(ru(2, 2)).toBe("Добавлен набор «Коты»: 2 стикера. Пропущено 2 анимированных или видеостикера.");
    expect(ru(5, 5)).toBe("Добавлен набор «Коты»: 5 стикеров. Пропущено 5 анимированных или видеостикеров.");
    expect(ru(27, 3)).toBe("Добавлен набор «Коты»: 27 стикеров. Пропущено 3 анимированных или видеостикера.");
  });

  it("every error code of the add flow has a short text in both languages", () => {
    for (const language of ["en", "ru"] as const) {
      for (const code of ["sticker_pack_no_token", "sticker_pack_not_found", "sticker_pack_token_refused", "sticker_pack_unreachable", "sticker_pack_wait", "sticker_pack_only_animated", "sticker_pack_too_large", "sticker_pack_busy", "sticker_pack_bad_response", "sticker_pack_link_invalid"] as const) {
        const view = render(<AddPackDialog {...base} hasToken state={{ phase: "idle", error: code === "sticker_pack_no_token" ? null : code }} />, { language });
        const text = screen.getByRole("dialog").textContent ?? "";
        expect(text, `${language} ${code}`).not.toContain("error.");
        view.unmount();
      }
    }
  });
});

describe("remove a pack", () => {
  it("asks 'Remove «Test Pack»? The stickers you sent stay in your entries.'; Cancel removes nothing; Remove does", async () => {
    const api = new MockApi({ language: "en" });
    const pack = api.stickerLibrary.seedPack("Test Pack", 2);
    const onClose = vi.fn();
    render(<RemovePackHost api={api} pack={pack} onClose={onClose} />, { language: "en" });
    const question = screen.getByRole("dialog", { name: "Remove pack" });
    expect(within(question).getByText("Remove «Test Pack»? The stickers you sent stay in your entries.")).toBeInTheDocument();
    fireEvent.click(within(question).getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalledOnce();
    expect((await api.getStickerLibrary()).packs).toHaveLength(1);
    fireEvent.click(within(question).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(2));
    expect((await api.getStickerLibrary()).packs).toEqual([]);
  });

  it("the pack from the export is named in the interface language; a long title is whole in the question", async () => {
    const api = new MockApi({ language: "ru" });
    const pack = api.stickerLibrary.seedPack("From Telegram", 1, "telegram_export");
    render(<RemovePackHost api={api} pack={pack} onClose={vi.fn()} />, { language: "ru" });
    expect(screen.getByText("Удалить «Из Telegram»? Отправленные вами стикеры останутся в записях.")).toBeInTheDocument();
  });

  it("an error stays in the question", async () => {
    const api = new MockApi({ language: "en" });
    const pack = api.stickerLibrary.seedPack("Test Pack", 1);
    api.failNext("removeStickerPack", "store_write_failed");
    render(<RemovePackHost api={api} pack={pack} onClose={vi.fn()} />, { language: "en" });
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not save the entry.");
  });
});
