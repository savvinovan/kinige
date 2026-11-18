import { describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import { act, fireEvent, render, screen, waitFor } from "../../test/render";
import { MainWindow } from "./MainWindow";

// What a drop can carry (KINIG-133): files, a picture with no file (dragged from Telegram or a browser), or nothing the app
// understands. The drop event comes from Rust as a token with no paths and no bytes; the window claims it with `attachDropped`.
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const MIB = 1024 * 1024;

function setup(language: "ru" | "en" = "en") {
  const api = new MockApi({ now: NOW, language });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language });
  return api;
}

const field = () => screen.findByRole("textbox", { name: /Новая заметка|New note/ });

describe("MainWindow: what a drop carries", () => {
  it("a picture with no file becomes a tile, like a pasted image", async () => {
    const api = setup();
    await field();
    act(() => void api.emitImageDropped(180_000));
    await waitFor(() => expect(api.tilesOf(INBOX_ID)).toHaveLength(1));
    expect(api.tilesOf(INBOX_ID)[0]?.kind).toBe("image");
    expect(api.tilesOf(INBOX_ID)[0]?.filename).toMatch(/^pasted-/);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("a picture over the paste limit says so and attaches nothing", async () => {
    const api = setup();
    await field();
    act(() => void api.emitImageDropped(64 * MIB + 1));
    expect(await screen.findByRole("alert")).toHaveTextContent("larger than 64 MB");
    expect(api.tilesOf(INBOX_ID)).toEqual([]);
  });

  it("a drop with no file and no image says so instead of doing nothing", async () => {
    const api = setup();
    await field();
    act(() => void api.emitAttachmentsDropped([]));
    expect(await screen.findByRole("alert")).toHaveTextContent("Nothing in what you dropped could be attached");
    expect(api.tilesOf(INBOX_ID)).toEqual([]);
  });

  it("the message is in Russian too, and a good drop afterwards clears it", async () => {
    const api = setup("ru");
    await field();
    act(() => void api.emitAttachmentsDropped([]));
    expect(await screen.findByRole("alert")).toHaveTextContent("ничего не удалось прикрепить");
    act(() => void api.emitImageDropped(1024));
    await waitFor(() => expect(api.tilesOf(INBOX_ID)).toHaveLength(1));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("tells Rust whether a chat is open to receive a drop: yes in a chat, no in the trash", async () => {
    const api = setup();
    await field();
    await waitFor(() => expect(api.dropTarget).toBe(true));
    fireEvent.click(await screen.findByRole("button", { name: /^Trash|^Корзина/ }));
    await waitFor(() => expect(api.dropTarget).toBe(false));
    fireEvent.click(await screen.findByRole("button", { name: /^Inbox|^Входящие/ }));
    await waitFor(() => expect(api.dropTarget).toBe(true));
  });

  it("a token that was already claimed is refused, not attached twice", async () => {
    const api = setup();
    await field();
    const token = api.emitImageDropped(2048);
    await waitFor(() => expect(api.tilesOf(INBOX_ID)).toHaveLength(1));
    await expect(api.attachDropped(token, INBOX_ID)).rejects.toMatchObject({ code: "attachment_not_found" });
    expect(api.tilesOf(INBOX_ID)).toHaveLength(1);
  });
});
