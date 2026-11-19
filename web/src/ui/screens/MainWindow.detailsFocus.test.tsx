import { describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, mockAttachment } from "../../api/MockApi";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// KINIG-136 in the main window: the entry's own menu, where the focus goes when the dialog closes, and the spoiler's menu round trip.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

function openWith(api: MockApi) {
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "en" });
}

function entry(api: MockApi, id: string, bodyMd: string, ...names: string[]) {
  api.addMessageWithAttachments(
    { id, chatId: INBOX_ID, bodyMd, createdAt: NOW().getTime(), journalDate: "2026-10-21" },
    names.map((name, index) => mockAttachment(`${id}-${index}`, name)),
  );
}

const dialogOf = () => screen.findByRole("dialog", { name: "Attachment details" });
const gone = () => waitFor(() => expect(screen.queryByRole("dialog", { name: "Attachment details" })).toBeNull());

describe("the entry's own menu", () => {
  it("has Attachment details… for an entry with attachments, and it opens the dialog on the first one", async () => {
    const api = new MockApi({ now: NOW, language: "en" });
    entry(api, "m1", "Beach day", "IMG_0001.jpg", "IMG_0002.jpg");
    entry(api, "m2", "Only text");
    openWith(api);

    fireEvent.contextMenu(await screen.findByText("Beach day"));
    const item = await screen.findByRole("menuitem", { name: "Attachment details…" });
    fireEvent.click(item);
    const dialog = await dialogOf();
    expect(within(dialog).getByText("1 / 2")).toBeInTheDocument();
    expect(within(dialog).getByRole("textbox", { name: "Name" })).toHaveValue("IMG_0001");

    // The entry with no attachments has no such item.
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await gone();
    fireEvent.contextMenu(screen.getByText("Only text"));
    await screen.findByRole("menuitem", { name: "Copy" });
    expect(screen.queryByRole("menuitem", { name: "Attachment details…" })).toBeNull();
  });

  it("returns the focus to the photo that the dialog was about", async () => {
    const api = new MockApi({ now: NOW, language: "en" });
    entry(api, "m1", "Beach day", "IMG_0001.jpg");
    openWith(api);
    fireEvent.contextMenu(await screen.findByText("Beach day"));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Attachment details…" }));
    fireEvent.click(within(await dialogOf()).getByRole("button", { name: "Cancel" }));
    await gone();
    expect(screen.getByRole("button", { name: /^Photo IMG_0001\.jpg/ })).toHaveFocus();
  });
});

describe("the focus when the dialog closes", () => {
  it("goes to the tile's Details button, not to whatever had the focus before (a photo far up the feed, which would scroll the feed to it)", async () => {
    const api = new MockApi({ now: NOW, language: "en" });
    entry(api, "m1", "", "IMG_0001.jpg");
    openWith(api);
    const older = await screen.findByRole("button", { name: /^Photo IMG_0001\.jpg/ });
    api.pickedFiles = ["IMG_0002.jpg"];
    fireEvent.click(await screen.findByRole("button", { name: "Attach file" }));
    await screen.findByRole("list", { name: "Attachments to send" });

    // In WebKit a click does not focus a button, so the focus is still where it was (here: on the older photo).
    older.focus();
    const details = screen.getByRole("button", { name: "Details of IMG_0002.jpg" });
    fireEvent.click(details);
    fireEvent.click(within(await dialogOf()).getByRole("button", { name: "Cancel" }));
    await gone();
    expect(details).toHaveFocus();
    expect(older).not.toHaveFocus();
  });

  it("goes to the file card that the dialog was opened on from its own menu", async () => {
    const api = new MockApi({ now: NOW, language: "en" });
    entry(api, "m1", "", "menu.pdf");
    openWith(api);
    const card = await screen.findByRole("button", { name: /^menu\.pdf/ });
    fireEvent.contextMenu(card);
    fireEvent.click(await screen.findByRole("menuitem", { name: "Details…" }));
    fireEvent.click(within(await dialogOf()).getByRole("button", { name: "Cancel" }));
    await gone();
    expect(card).toHaveFocus();
  });
});

describe("the spoiler from the menu, round trip", () => {
  it("Hide as spoiler after Show without spoiler covers a photo that was revealed before", async () => {
    const api = new MockApi({ now: NOW, language: "en" });
    entry(api, "m1", "", "IMG_0001.jpg", "IMG_0002.jpg");
    openWith(api);
    const photo = () => screen.getByRole("button", { name: /^Photo IMG_0001\.jpg/ });

    fireEvent.contextMenu(await screen.findByRole("button", { name: /^Photo IMG_0001\.jpg/ }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Hide as spoiler" }));
    fireEvent.click(await screen.findByRole("button", { name: "Hidden photo, press to show" }));
    expect(await screen.findByRole("button", { name: "Hide the photo again" })).toBeInTheDocument();

    fireEvent.contextMenu(photo());
    fireEvent.click(await screen.findByRole("menuitem", { name: "Show without spoiler" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Hide the photo again" })).toBeNull());
    expect(photo()).toBeInTheDocument();

    // Hidden again: the cover is back, not the old "shown" state of the click before.
    fireEvent.contextMenu(photo());
    fireEvent.click(await screen.findByRole("menuitem", { name: "Hide as spoiler" }));
    expect(await screen.findByRole("button", { name: "Hidden photo, press to show" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Hide the photo again" })).toBeNull();
  });
});
