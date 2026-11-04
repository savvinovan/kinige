import { beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import { fireEvent, render, screen, waitFor } from "../../test/render";
import { MainWindow } from "./MainWindow";

// KINIG-47.1: a key press in the composer does not render the window, the feed or the plates; sending an entry renders
// only the new plate. Counted by wrapping components that the real ones render: `MessageActions` is drawn once per render of a
// `Message` (view mode), so its count is the count of real `Message` renders (the real `Message` stays memoised; the wrapper is not).
const renders = vi.hoisted(() => ({ message: vi.fn(), feed: vi.fn() }));

vi.mock("../message/MessageActions", async (importOriginal) => {
  const real = await importOriginal<typeof import("../message/MessageActions")>();
  const Counted = (props: React.ComponentProps<typeof real.MessageActions>) => {
    renders.message();
    return <real.MessageActions {...props} />;
  };
  return { ...real, MessageActions: Counted };
});

vi.mock("../feed/Feed", async (importOriginal) => {
  const real = await importOriginal<typeof import("../feed/Feed")>();
  const Counted = (props: React.ComponentProps<typeof real.Feed>) => {
    renders.feed();
    return <real.Feed {...props} />;
  };
  return { ...real, Feed: Counted };
});

const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const field = () => screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement;

function entries(count: number) {
  return Array.from({ length: count }, (_, index) =>
    mockMessage({ id: `m${index}`, bodyMd: `Запись ${index}`, createdAt: Date.UTC(2026, 9, 21, 1, index) }),
  );
}

async function openWindow(count: number) {
  const api = new MockApi({ now: NOW, language: "ru", messages: entries(count) });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />, { language: "ru" });
  await screen.findByText(`Запись ${count - 1}`);
  // Let the first renders (the card, chats, the draft request) settle before counting.
  await waitFor(() => expect(field()).toBeEnabled());
  await new Promise((resolve) => setTimeout(resolve, 50));
  return api;
}

describe("MainWindow: перерисовки ленты", () => {
  beforeEach(() => {
    renders.message.mockClear();
    renders.feed.mockClear();
  });

  it("набор 20 символов не перерисовывает ни ленту, ни плашки", async () => {
    await openWindow(40);
    const messageBefore = renders.message.mock.calls.length;
    const feedBefore = renders.feed.mock.calls.length;
    let text = "";
    for (let index = 0; index < 20; index++) {
      text += "а";
      fireEvent.change(field(), { target: { value: text } });
    }
    expect(field().value).toBe(text);
    expect(renders.message).toHaveBeenCalledTimes(messageBefore);
    expect(renders.feed).toHaveBeenCalledTimes(feedBefore);
  });

  it("после отправки перерисовывается только новая плашка", async () => {
    await openWindow(40);
    const messageBefore = renders.message.mock.calls.length;
    fireEvent.change(field(), { target: { value: "новая запись" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByText("новая запись");
    await waitFor(() => expect(field().value).toBe(""));
    // The new plate is drawn once or twice (mount); the 40 old ones are not.
    expect(renders.message.mock.calls.length - messageBefore).toBeLessThanOrEqual(2);
  });
});
