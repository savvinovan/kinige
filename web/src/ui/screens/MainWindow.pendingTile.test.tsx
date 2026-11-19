import { describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// A click on a picture tile above the field opens the viewer on the unsent pictures (KINIG-135).
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

async function withTiles(names: string[]) {
  const api = new MockApi({ now: NOW, language: "en" });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "en" });
  api.pickedFiles = names;
  fireEvent.click(await screen.findByRole("button", { name: "Attach file" }));
  await screen.findByRole("list", { name: "Attachments to send" });
  return api;
}

describe("MainWindow: плитка фото над полем открывает просмотр", () => {
  it("клик по плитке: окно просмотра только с фото черновика, счётчик, без «Открыть в программе»; Escape закрывает, плитки на месте", async () => {
    const api = await withTiles(["a.jpg", "menu.pdf", "b.jpg"]);
    fireEvent.click(await screen.findByRole("button", { name: /^Photo b\.jpg/ }));
    const dialog = await screen.findByRole("dialog", { name: "Photo viewer" });
    expect(within(dialog).getByText("2 / 2")).toBeInTheDocument();
    // Rust opens only sent files in the system app: an unsent photo has no such button.
    expect(within(dialog).queryByRole("button", { name: "Open in default app" })).toBeNull();
    expect(api.openedAttachments).toEqual([]);
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Photo viewer" })).toBeNull());
    expect(api.tilesOf(INBOX_ID)).toHaveLength(3);
  });

  it("плитка файла не кнопка просмотра: у неё только ×", async () => {
    await withTiles(["menu.pdf"]);
    const tiles = screen.getByRole("list", { name: "Attachments to send" });
    expect(within(tiles).getAllByRole("button").map((button) => button.getAttribute("aria-label"))).toEqual(["Remove menu.pdf"]);
  });
});
