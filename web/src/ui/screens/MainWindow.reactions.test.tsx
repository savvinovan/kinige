import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Slice 5a on the real `MainWindow` and `MockApi`: a click on an emoji of the hover bar puts a chip under the text, a click on the chip removes it,
// the quick set follows the reactions put, and a refusal is an error line above the field (no toast, nothing else moves).
const NOW = () => new Date(Date.UTC(2026, 10, 18, 18, 5));
const AT = Date.UTC(2026, 10, 18, 5, 0);
const HEART = "❤\u{FE0F}";

function setup(messages = [mockMessage({ id: "m1", bodyMd: "Сдал экзамен!", createdAt: AT, journalDate: "2026-11-18" })]) {
  const api = new MockApi({ now: NOW, messages });
  render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
  return api;
}

const wrap = (text: string) => screen.getByText(text).closest(".message-wrap") as HTMLElement;
const bar = (text: string) => within(wrap(text)).getByRole("toolbar", { name: "Действия с сообщением" });
const barEmoji = (text: string) => [...bar(text).querySelectorAll<HTMLButtonElement>(".reaction-button")];
const chips = (text: string) => within(wrap(text)).queryAllByRole("button", { name: /Снять$/ });

describe("реакции в ленте", () => {
  it("щелчок по ❤️ в ряду ставит чип под текстом внутри плашки; повторный щелчок по кнопке снимает; чип снимает тоже", async () => {
    const api = setup();
    await screen.findByText("Сдал экзамен!");
    expect(barEmoji("Сдал экзамен!").map((b) => b.textContent)).toEqual([HEART, "😂", "😢"]);
    expect(chips("Сдал экзамен!")).toHaveLength(0);

    fireEvent.click(barEmoji("Сдал экзамен!")[0] as HTMLElement);
    await waitFor(() => expect(chips("Сдал экзамен!")).toHaveLength(1));
    expect(wrap("Сдал экзамен!").querySelector("article .reaction-row")).toBeInTheDocument();
    expect(barEmoji("Сдал экзамен!")[0]).toHaveAttribute("aria-pressed", "true");
    expect(api.calls).toContain(`setMessageReaction m1 ${HEART} true`);
    // No toast and no status line: the only change is the chip.
    expect(screen.queryByRole("alert")).toBeNull();

    fireEvent.click(barEmoji("Сдал экзамен!")[0] as HTMLElement);
    await waitFor(() => expect(chips("Сдал экзамен!")).toHaveLength(0));

    fireEvent.click(barEmoji("Сдал экзамен!")[1] as HTMLElement);
    await waitFor(() => expect(chips("Сдал экзамен!")).toHaveLength(1));
    fireEvent.click(chips("Сдал экзамен!")[0] as HTMLElement);
    await waitFor(() => expect(chips("Сдал экзамен!")).toHaveLength(0));
  });

  it("быстрый набор идёт за поставленным, но пока указатель на записи, порядок кнопок не сдвигается из-под него", async () => {
    const api = setup();
    await screen.findByText("Сдал экзамен!");
    const zone = wrap("Сдал экзамен!").closest(".feed__item") as HTMLElement;
    act(() => zone.dispatchEvent(new Event("pointerenter")));
    const labels = () => barEmoji("Сдал экзамен!").map((b) => b.textContent);
    expect(labels()).toEqual([HEART, "😂", "😢"]);

    fireEvent.click(barEmoji("Сдал экзамен!")[1] as HTMLElement);
    await waitFor(() => expect(chips("Сдал экзамен!")).toHaveLength(1));
    await waitFor(async () => expect((await api.getSettings()).quickReactions).toEqual(["😂", HEART, "😢"]));
    // The latest reaction is first for the next hover, but the bar under the pointer keeps its order: the next click lands on the same emoji.
    expect(labels()).toEqual([HEART, "😂", "😢"]);

    act(() => zone.dispatchEvent(new Event("pointerleave")));
    await waitFor(() => expect(labels()).toEqual(["😂", HEART, "😢"]));
  });

  it("отказ (десять живых): строка ошибки над полем, ничего не поставлено", async () => {
    const api = setup();
    await screen.findByText("Сдал экзамен!");
    api.failNext("setMessageReaction", "reaction_limit");
    fireEvent.click(barEmoji("Сдал экзамен!")[1] as HTMLElement);
    expect(await screen.findByRole("alert")).toHaveTextContent("нельзя поставить ещё одну реакцию");
    expect(chips("Сдал экзамен!")).toHaveLength(0);
  });

  it("запись из корзины и запись только для чтения чипы не меняют: у них нет кнопок реакций", async () => {
    setup([mockMessage({ id: "m1", bodyMd: "Только чтение", createdAt: AT, journalDate: "2026-11-18", readOnly: true })]);
    await screen.findByText("Только чтение");
    for (const button of barEmoji("Только чтение")) expect(button).toHaveAttribute("aria-disabled", "true");
  });
});
