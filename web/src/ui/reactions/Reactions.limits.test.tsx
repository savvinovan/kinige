import { afterEach, beforeAll, afterAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactionView } from "../../api/generated/ReactionView";
import { mockMessage } from "../../api/mockData";
import { installEmojiLayoutStub, StubEmojiData } from "../../test/emoji";
import { fireEvent, render, screen, within } from "../../test/render";
import { Message } from "../message/Message";
import { CARD_TODAY, detailedYear } from "../onthisday/demoCard";
import { OnThisDayCard } from "../onthisday/OnThisDayCard";
import { ReactionChoicesProvider } from "./ReactionsContext";

// Slice 5a, the full entry and the open picker: at 10 live reactions "More" of the small popover and the menu item "Add reaction…" are shut and say why, as the
// bar's picker button is; while a picker is open its opener stays on screen (a panel without an owner on the screen was found in review).

const HEART = "❤\u{FE0F}";
const sentAt = new Date(2026, 10, 18, 9, 5);
const TEN = Array.from({ length: 10 }, (_, i): ReactionView => ({ emoji: String.fromCodePoint(0x1f600 + i), putAt: 0, putTz: "UTC", yearsLater: 0 }));
const REASON = "На записи не больше 10 реакций";

let restoreLayout: () => void;
beforeAll(() => {
  restoreLayout = installEmojiLayoutStub();
});
afterAll(() => restoreLayout());
beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(Date.UTC(2026, 10, 25, 12, 0)));
});
afterEach(() => vi.useRealTimers());

function entry(reactions: ReactionView[], counts?: { key: number; live: number }, keysHad: string[] = []) {
  return render(
    <StubEmojiData>
      <ReactionChoicesProvider value={{ quick: [HEART, "😂", "😢"], recent: [] }}>
        <Message
          markdown="Сдал экзамен!"
          sentAt={sentAt}
          timeZone="UTC"
          reactions={reactions}
          reactionKeysHad={keysHad}
          reactionCounts={counts}
          onReact={vi.fn()}
          onEdit={vi.fn()}
          onDelete={vi.fn()}
        />
      </ReactionChoicesProvider>
    </StubEmojiData>,
  );
}

function card(reactions: ReactionView[]) {
  const year = detailedYear(2025, ["Сдал экзамен!"]);
  const first = year.entries[0];
  if (first) first.message = mockMessage({ ...first.message, reactions });
  return render(
    <ReactionChoicesProvider value={{ quick: [HEART, "😂", "😢"], recent: [] }}>
      <OnThisDayCard card={{ today: CARD_TODAY, years: [year] }} onOpen={vi.fn()} onReply={vi.fn()} onDismiss={vi.fn()} onReact={vi.fn()} />
    </ReactionChoicesProvider>,
  );
}

describe("запись с десятью реакциями", () => {
  it("«Ещё» малого окна закрыто и говорит почему; щелчок по нему не открывает полный выбор", async () => {
    card(TEN);
    fireEvent.click(screen.getAllByRole("button", { name: /Поставить реакцию на запись/ })[0] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: "Выбор реакции" });
    const more = within(dialog).getByRole("button", { name: "Ещё эмодзи" });
    expect(more).toHaveAttribute("aria-disabled", "true");
    expect(more).toHaveAttribute("title", `Ещё эмодзи. ${REASON}`);
    fireEvent.click(more);
    expect(within(dialog).queryByRole("searchbox")).toBeNull();
  });

  it("у записи без десяти «Ещё» открыто и полный выбор открывается", async () => {
    card(TEN.slice(0, 9));
    fireEvent.click(screen.getAllByRole("button", { name: /Поставить реакцию на запись/ })[0] as HTMLElement);
    const dialog = await screen.findByRole("dialog", { name: "Выбор реакции" });
    const more = within(dialog).getByRole("button", { name: "Ещё эмодзи" });
    expect(more).not.toHaveAttribute("aria-disabled");
    fireEvent.click(more);
    expect(await within(dialog).findByRole("searchbox")).toBeInTheDocument();
  });

  it("пункт меню «Поставить реакцию…» закрыт и говорит почему; выбор его ничего не открывает", () => {
    const { container } = entry(TEN);
    fireEvent.keyDown(container.querySelector("article") as HTMLElement, { key: "F10", shiftKey: true });
    const item = screen.getByRole("menuitem", { name: "Поставить реакцию…" });
    expect(item).toHaveAttribute("aria-disabled", "true");
    expect(item).toHaveAttribute("title", `Поставить реакцию…. ${REASON}`);
    fireEvent.click(item);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("у записи с девятью пункт меню открыт", () => {
    const { container } = entry(TEN.slice(0, 9));
    fireEvent.keyDown(container.querySelector("article") as HTMLElement, { key: "F10", shiftKey: true });
    const item = screen.getByRole("menuitem", { name: "Поставить реакцию…" });
    expect(item).not.toHaveAttribute("aria-disabled");
    expect(item).not.toHaveAttribute("title");
  });
});

describe("пока открыт выбор, его кнопка видна", () => {
  it("ряд записи остаётся на экране, пока открыто окно выбора, и снимается, когда оно закрылось и указателя нет", async () => {
    const { container } = entry([]);
    const wrap = container.querySelector(".message-wrap") as HTMLElement;
    expect(wrap).not.toHaveClass("message-wrap--actions");
    fireEvent.click(within(wrap).getByRole("button", { name: "Поставить реакцию" }));
    await screen.findByRole("dialog", { name: "Выбор реакции" });
    expect(wrap).toHaveClass("message-wrap--actions");
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    // The opener got focus back, and the bar is shown by the focus inside the entry, not by the open panel.
    expect(wrap).not.toHaveClass("message-wrap--actions");
  });

  it("кнопка со смайлом карточки видна, пока открыто её окно", async () => {
    card([]);
    const opener = screen.getAllByRole("button", { name: /Поставить реакцию на запись/ })[0] as HTMLElement;
    expect(opener).not.toHaveClass("on-this-day__react--open");
    fireEvent.click(opener);
    await screen.findByRole("dialog", { name: "Выбор реакции" });
    expect(opener).toHaveClass("on-this-day__react--open");
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(opener).not.toHaveClass("on-this-day__react--open");
  });
});

describe("элементы группы, которых не видно, занимают места так же, как в хранилище", () => {
  const barButton = (name: RegExp | string) => screen.getAllByRole("button", { name }).find((b) => b.closest(".message-actions")) as HTMLElement;

  it("девять видимых реакций и один скрытый живой элемент: это десять, кнопки добавления закрыты, как закроет хранилище", () => {
    entry(TEN.slice(0, 9), { key: 10, live: 10 });
    expect(barButton("Поставить реакцию")).toHaveAttribute("aria-disabled", "true");
    expect(barButton("Реакция 😢")).toHaveAttribute("aria-disabled", "true");
    entry(TEN.slice(0, 9), { key: 9, live: 9 });
    expect(screen.getAllByRole("button", { name: "Реакция 😢" })[1]).not.toHaveAttribute("aria-disabled");
  });

  it("тридцать два ключа с невидимыми: новый эмодзи закрыт, а уже бывший ещё переключается", () => {
    entry([], { key: 32, live: 0 }, ["❤"]);
    const sad = screen.getByRole("button", { name: "Реакция 😢" });
    expect(sad).toHaveAttribute("aria-disabled", "true");
    expect(sad.getAttribute("title")).toContain("На этой записи уже были 32 разные реакции");
    expect(screen.getByRole("button", { name: `Реакция ${HEART}` })).not.toHaveAttribute("aria-disabled");
  });

  it("число меньше увиденного не обманывает: берётся большее (запись без счётчиков или с нулями)", () => {
    entry(TEN, { key: 0, live: 0 });
    expect(screen.getByRole("button", { name: "Реакция 😢" })).toHaveAttribute("aria-disabled", "true");
  });
});

describe("зона, которой нет на этом устройстве, читается как UTC, как в Rust", () => {
  it("всплывающая подсказка чипа называет день по UTC", () => {
    // 2026-11-19 23:30 UTC with the 04:00 boundary is the day of 19 November in UTC; a window zone far from UTC would say another one.
    const chip: ReactionView = { emoji: HEART, putAt: Date.UTC(2026, 10, 19, 23, 30), putTz: "Not/AZone", yearsLater: 0 };
    entry([chip]);
    expect(screen.getByRole("button", { name: /^❤️?, Поставлена 19 ноября 2026/ })).toBeInTheDocument();
  });
});
