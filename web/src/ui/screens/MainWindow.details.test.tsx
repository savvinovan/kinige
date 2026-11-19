import { describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, mockAttachment } from "../../api/MockApi";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// KINIG-136 in the main window: "Details" on a tile, the cover on the sent entry, the context menus of an attachment and of the entry.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

function openWith(api: MockApi) {
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "en" });
}

describe("MainWindow: attachment details", () => {
  it("Details on a tile: the dialog saves only what changed, the tile turns into a cover, the entry is sent covered and the neighbour is not", async () => {
    const api = new MockApi({ now: NOW, language: "en" });
    openWith(api);
    api.pickedFiles = ["IMG_0001.jpg", "IMG_0002.jpg", "menu.pdf"];
    fireEvent.click(await screen.findByRole("button", { name: "Attach file" }));
    await screen.findByRole("list", { name: "Attachments to send" });

    fireEvent.click(screen.getByRole("button", { name: "Details of IMG_0001.jpg" }));
    const dialog = await screen.findByRole("dialog", { name: "Attachment details" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Name" }), { target: { value: "beach" } });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Description" }), { target: { value: "Beach, July" } });
    fireEvent.click(within(dialog).getByRole("checkbox", { name: /Hide as spoiler/ }));
    api.calls.length = 0;
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Attachment details" })).toBeNull());
    expect(api.calls).toEqual([expect.stringMatching(/^setDraftAttachmentDetails /)]);
    expect(api.tilesOf(INBOX_ID).map((tile) => [tile.name, tile.description, tile.spoiler])).toEqual([
      ["beach.jpg", "Beach, July", true],
      ["IMG_0002.jpg", null, false],
      ["menu.pdf", null, false],
    ]);
    // The tile is covered now, and the neighbour is untouched.
    const tiles = screen.getByRole("list", { name: "Attachments to send" });
    expect(within(tiles).getByRole("button", { name: "Hidden photo, press to show" })).toBeInTheDocument();
    expect(within(tiles).getByRole("button", { name: /^Photo IMG_0002\.jpg/ })).toBeInTheDocument();

    // Save with nothing changed does not call Rust.
    fireEvent.click(screen.getByRole("button", { name: "Details of menu.pdf" }));
    api.calls.length = 0;
    fireEvent.click(await screen.findByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Attachment details" })).toBeNull());
    expect(api.calls).toEqual([]);

    // Sent, the photo is still hidden (the cover is the entry's, not the tile's) and the details are kept.
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(async () => expect((await api.listMessages(INBOX_ID)).messages).toHaveLength(1));
    const [sent] = (await api.listMessages(INBOX_ID)).messages;
    expect(sent?.attachments.map((a) => [a.name, a.spoiler])).toEqual([
      ["beach.jpg", true],
      ["IMG_0002.jpg", false],
      ["menu.pdf", false],
    ]);
    expect(await screen.findByRole("button", { name: "Hidden photo, press to show" })).toBeInTheDocument();
  });

  it("the menus: Hide as spoiler covers a photo at once and Show without spoiler uncovers it; Details… opens the dialog on a file card", async () => {
    const photo = mockAttachment("p1", "IMG_0001.jpg");
    const second = mockAttachment("p2", "IMG_0002.jpg");
    const pdf = mockAttachment("f1", "menu.pdf");
    const api = new MockApi({ now: NOW, language: "en" });
    api.addMessageWithAttachments({ id: "m1", chatId: INBOX_ID, bodyMd: "", createdAt: NOW().getTime(), journalDate: "2026-10-21" }, [photo, second, pdf]);
    openWith(api);

    fireEvent.contextMenu(await screen.findByRole("button", { name: /^Photo IMG_0001\.jpg/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Hide as spoiler" }));
    // Covered at once, and the neighbour keeps its picture.
    expect(await screen.findByRole("button", { name: "Hidden photo, press to show" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Photo IMG_0002\.jpg/ })).toBeInTheDocument();
    expect((await api.listMessages(INBOX_ID)).messages[0]?.attachments[0]?.spoiler).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Hidden photo, press to show" }));
    expect(screen.getByRole("button", { name: "Hide the photo again" })).toBeInTheDocument();
    fireEvent.contextMenu(screen.getByRole("button", { name: /^Photo IMG_0001\.jpg/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Show without spoiler" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Hide the photo again" })).toBeNull());
    expect((await api.listMessages(INBOX_ID)).messages[0]?.attachments[0]?.spoiler).toBe(false);

    // The file card's own menu has Details… and no spoiler item; the dialog opens on the card and knows the entry's other attachments.
    fireEvent.contextMenu(screen.getByRole("button", { name: /^menu\.pdf/ }));
    const items = await screen.findAllByRole("menuitem");
    expect(items.map((item) => item.textContent)).toEqual(["Details…"]);
    fireEvent.click(items[0] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: "Attachment details" });
    expect(within(dialog).getByText("3 / 3")).toBeInTheDocument();
    expect(within(dialog).queryByRole("checkbox")).toBeNull();
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Description" }), { target: { value: "Lunch menu" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Attachment details" })).toBeNull());
    expect(await screen.findByRole("button", { name: /Lunch menu/ })).toBeInTheDocument();
  });

  it("a rule broken in the dialog keeps it open with Rust's error and the fields as typed", async () => {
    const api = new MockApi({ now: NOW, language: "en" });
    api.addMessageWithAttachments({ id: "m1", chatId: INBOX_ID, bodyMd: "x", createdAt: NOW().getTime(), journalDate: "2026-10-21" }, [mockAttachment("p1", "IMG_0001.jpg")]);
    openWith(api);
    fireEvent.contextMenu(await screen.findByRole("button", { name: /^Photo IMG_0001\.jpg/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Details…" }));
    const dialog = await screen.findByRole("dialog", { name: "Attachment details" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Name" }), { target: { value: "a/b" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("That name or description is not accepted");
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("a/b");
    expect(screen.getByRole("dialog", { name: "Attachment details" })).toBeInTheDocument();
  });
});
