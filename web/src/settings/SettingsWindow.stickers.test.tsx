import { describe, expect, it } from "vitest";
import type { MockApi } from "../api/MockApi";
import { fireEvent, screen, waitFor, within } from "../test/render";
import { openWindow } from "../test/prefsWindow";

// The Stickers section of the Settings window (slice 5c): the bot token. The window reads only the flag, never the token.

const TOKEN = "123456:ABC-def_1";
const field = () => screen.getByLabelText("Telegram bot token") as HTMLInputElement;
const openStickers = async (api: MockApi) => {
  fireEvent.click(screen.getByRole("button", { name: "Stickers" }));
  await screen.findByRole("heading", { name: "Stickers", level: 2 });
  return api;
};

async function open(saved = false) {
  const api = await openWindow({}, "en");
  if (saved) await api.setTelegramBotToken(TOKEN);
  return openStickers(api);
}

describe("Settings → Stickers", () => {
  it("the flag is read only when the section is open, and nothing else about the token is ever asked for", async () => {
    const api = await openWindow({}, "en");
    expect(api.calls.some((call) => call.includes("TelegramToken") || call.includes("TelegramBotToken"))).toBe(false);
    await openStickers(api);
    await screen.findByLabelText("Telegram bot token");
    expect(api.calls.filter((call) => call.includes("elegram"))).toEqual(["getTelegramTokenState"]);
  });

  it("the explanation line and a password-style field with Save; the two notes are there", async () => {
    await open();
    expect(screen.getByText("Kinige uses your own bot only to download the packs you name. The token is stored on this computer and sent only to Telegram.")).toBeInTheDocument();
    expect(field()).toHaveAttribute("type", "password");
    expect(field()).toHaveAttribute("autocomplete", "off");
    expect(screen.getByText(/What Telegram learns when you add a pack: the bot token, the name of the pack, your IP address/)).toBeInTheDocument();
    expect(screen.getByText(/After an update macOS may ask once to allow Kinige to use it/)).toBeInTheDocument();
  });

  it("Save sends the token and clears the field at once; then 'Token saved' with Remove replaces the field", async () => {
    const api = await open();
    fireEvent.change(field(), { target: { value: `  ${TOKEN}  ` } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    // The field is empty before Rust has even answered: the token does not stay in the window.
    expect(field().value).toBe("");
    expect(await screen.findByText("Token saved")).toBeInTheDocument();
    expect(screen.queryByLabelText("Telegram bot token")).toBeNull();
    expect((await api.getTelegramTokenState()).saved).toBe(true);
    // The token is nowhere in the page.
    expect(document.body.innerHTML).not.toContain("ABC-def");
    fireEvent.click(screen.getByRole("button", { name: "Remove the saved bot token" }));
    await waitFor(() => expect(field()).toBeInTheDocument());
    expect((await api.getTelegramTokenState()).saved).toBe(false);
  });

  it("Save with an empty field does nothing; Enter in the field saves", async () => {
    const api = await open();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.calls.some((call) => call.startsWith("setTelegramBotToken"))).toBe(false);
    fireEvent.change(field(), { target: { value: TOKEN } });
    fireEvent.submit(field().closest("form") as HTMLFormElement);
    expect(await screen.findByText("Token saved")).toBeInTheDocument();
  });

  it.each([
    ["telegram_token_invalid", "That is not a bot token. A token looks like 123456:ABC-def and comes from Telegram's BotFather."],
    ["secret_store_unavailable", "The system keychain is not available, so the token was not saved."],
    ["secret_store_denied", "macOS asked for access to the saved token and it was not given. Press “Remove”, then save the token again."],
  ] as const)("%s is one line under the field, and the field is empty again", async (code, text) => {
    const api = await open();
    api.failNext("setTelegramBotToken", code);
    fireEvent.change(field(), { target: { value: TOKEN } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(text);
    expect(field().value).toBe("");
  });

  it("a token of the wrong shape is refused by the mock as Rust refuses it", async () => {
    await open();
    fireEvent.change(field(), { target: { value: "not-a-token" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("That is not a bot token.");
    expect(screen.queryByText("Token saved")).toBeNull();
  });

  it("with a token already saved the section opens on 'Token saved'; a denied removal says what to do", async () => {
    const api = await open(true);
    expect(await screen.findByText("Token saved")).toBeInTheDocument();
    api.failNext("clearTelegramBotToken", "secret_store_denied");
    fireEvent.click(screen.getByRole("button", { name: "Remove the saved bot token" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Press “Remove”, then save the token again.");
    // Still saved: the item was not removed.
    expect(within(screen.getByRole("main")).getByText("Token saved")).toBeInTheDocument();
  });

  it("Russian: the labels and the section name", async () => {
    const api = await openWindow({}, "ru");
    fireEvent.click(screen.getByRole("button", { name: "Стикеры" }));
    expect(await screen.findByLabelText("Токен бота Telegram")).toHaveAttribute("type", "password");
    expect(screen.getByRole("button", { name: "Сохранить" })).toBeInTheDocument();
    expect(api.calls.filter((call) => call.includes("elegram"))).toEqual(["getTelegramTokenState"]);
  });
});
