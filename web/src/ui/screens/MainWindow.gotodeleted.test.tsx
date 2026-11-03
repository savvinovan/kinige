import { describe, expect, it } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// A jump to an entry that is no longer in place (KINIG-45): a reply quote, search and the "On this day" card ask Rust for the
// page around the entry, and the reply is `message_not_found`. This is not a feed load error: the window stays, the feed
// stands as it was, the error is a quiet line above the composer.

const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const QUOTE = "Ответ на запись 20 октября 2026";

async function openWithReply() {
  const api = new MockApi({
    now: NOW,
    messages: [
      mockMessage({ id: "old", bodyMd: "исходная мысль", journalDate: "2026-10-20", createdAt: Date.UTC(2026, 9, 20, 12) }),
    ],
  });
  await api.sendMessage(INBOX_ID, "ответ на мысль", "old");
  const view = render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
  await screen.findByText("ответ на мысль");
  return { api, ...view };
}

describe("Переход к удалённой записи", () => {
  it("цитата ответа ведёт на удалённую запись: окно и лента на месте, над полем тихая ошибка, цитата обновилась", async () => {
    const { container } = await openWithReply();
    const plate = container.querySelector<HTMLElement>('[data-message-id="old"] article');
    fireEvent.contextMenu(plate as HTMLElement);
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Удалить" }));
    await screen.findByRole("button", { name: "Вернуть удалённую запись" });

    // In the loaded feed the quote is still "live": a click on it asks for the page around an entry that Rust no longer has.
    fireEvent.click(screen.getByRole("button", { name: QUOTE }));

    expect(await screen.findByText(/Запись не найдена/)).toBeInTheDocument();
    expect(screen.queryByText(/Не удалось загрузить/)).toBeNull();
    expect(screen.getByRole("textbox", { name: "Новая заметка" })).toBeInTheDocument();
    expect(screen.getByText("ответ на мысль")).toBeInTheDocument();
    // The quote is reread together with the feed: the entry is now in the trash, the quote is no longer a button.
    await waitFor(() => expect(screen.queryByRole("button", { name: QUOTE })).toBeNull());
    expect(screen.getByText("Запись в корзине")).toBeInTheDocument();
  });
});
