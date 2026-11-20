import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactionView } from "../../api/generated/ReactionView";
import type { SearchHit } from "../../api/generated/SearchHit";
import type { TrashItem } from "../../api/generated/TrashItem";
import { mockMessage } from "../../api/mockData";
import { fireEvent, render, screen, within } from "../../test/render";
import { detailedYear, CARD_TODAY } from "../onthisday/demoCard";
import { OnThisDayCard } from "../onthisday/OnThisDayCard";
import { NO_FILTERS, SearchResults } from "../search/SearchResults";
import { TrashView } from "../trash/TrashView";
import { ReactionChoicesProvider } from "./ReactionsContext";

// Slice 5a outside the feed: the "On this day" card (the smile button, the popover, the chips), the trash and search rows (chips that are only labels).

const HEART = "❤\u{FE0F}";
const chip = (emoji: string, yearsLater = 0): ReactionView => ({ emoji, putAt: Date.UTC(2026, 9, 31, 10, 0), putTz: "UTC", yearsLater });

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(Date.UTC(2026, 10, 25, 12, 0)));
});
afterEach(() => vi.useRealTimers());

function card(reactions: ReactionView[] = [], readOnly = false) {
  const year = detailedYear(2025, ["Сдал экзамен!"]);
  const first = year.entries[0];
  if (first) first.message = mockMessage({ ...first.message, reactions, readOnly });
  return { today: CARD_TODAY, years: [year] };
}

function showCard(reactions: ReactionView[] = [], { onReact = vi.fn() as ((id: string, emoji: string, on: boolean) => void) | null, readOnly = false } = {}) {
  render(
    <ReactionChoicesProvider value={{ quick: [HEART, "😂", "😢"], recent: [] }}>
      <OnThisDayCard card={card(reactions, readOnly)} onOpen={vi.fn()} onReply={vi.fn()} onDismiss={vi.fn()} onReact={onReact ?? undefined} />
    </ReactionChoicesProvider>,
  );
  return onReact;
}

describe("карточка «В этот день»: реакции", () => {
  it("кнопка со смайлом рядом с «Ответить» открывает малое окно: три эмодзи и «Ещё»; выбор ставит реакцию на запись и возвращает фокус на кнопку", () => {
    const onReact = showCard();
    const button = screen.getByRole("button", { name: "Поставить реакцию на запись 31 октября 2025" });
    expect(document.activeElement).not.toBe(button);
    fireEvent.click(button);
    const dialog = screen.getByRole("dialog", { name: "Выбор реакции" });
    expect(within(dialog).getAllByRole("button").map((b) => b.textContent)).toEqual([HEART, "😂", "😢", "Ещё"]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Реакция 😢" }));
    expect(onReact).toHaveBeenCalledExactlyOnceWith("m-2025-0", "😢", true);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(button).toHaveFocus();
  });

  it("Esc закрывает окно и ничего не ставит", () => {
    const onReact = showCard();
    const button = screen.getByRole("button", { name: /^Поставить реакцию/ });
    fireEvent.click(button);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onReact).not.toHaveBeenCalled();
    expect(button).toHaveFocus();
  });

  it("чип под записью карточки: «🥲 1 г.», подсказка со словами «год спустя»; щелчок снимает реакцию", () => {
    const onReact = showCard([chip("🥲", 1)]);
    const chipButton = screen.getByRole("button", { name: /^🥲, Поставлена 31 октября 2026 · 1 год спустя\. Снять$/ });
    expect(chipButton).toHaveTextContent("1 г.");
    fireEvent.click(chipButton);
    expect(onReact).toHaveBeenCalledExactlyOnceWith("m-2025-0", "🥲", false);
  });

  it("уже нажатая реакция в малом окне нажата (aria-pressed): щелчок снимает её", () => {
    const onReact = showCard([chip(HEART)]);
    fireEvent.click(screen.getByRole("button", { name: /^Поставить реакцию/ }));
    const heart = screen.getByRole("button", { name: `Реакция ${HEART}` });
    expect(heart).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(heart);
    expect(onReact).toHaveBeenCalledExactlyOnceWith("m-2025-0", HEART, false);
  });

  it("запись только для чтения: кнопки нет, чипы — подписи без роли кнопки", () => {
    showCard([chip("🥲")], { readOnly: true });
    expect(screen.queryByRole("button", { name: /^Поставить реакцию/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Снять$/ })).toBeNull();
    expect(screen.getByRole("img", { name: /^🥲, / })).toBeInTheDocument();
  });

  it("без обработчика карточка прежняя: ни кнопки, ни кнопочных чипов", () => {
    showCard([chip("🥲")], { onReact: null });
    expect(screen.queryByRole("button", { name: /^Поставить реакцию/ })).toBeNull();
    expect(screen.getByRole("img", { name: /^🥲, / })).toBeInTheDocument();
  });
});

describe("корзина и поиск: чипы показаны и не меняются", () => {
  it("корзина: чипы подписями без роли кнопки, рядом только «Восстановить» и «Удалить навсегда»", () => {
    const item: TrashItem = {
      message: mockMessage({ id: "t1", bodyMd: "в корзине", reactions: [chip(HEART), chip("🔥")], reactionKeysHad: ["❤", "🔥"] }),
      chatTitle: "Книги",
      chatIcon: "📚",
      chatIsInbox: false,
      chatColor: "violet",
      deletedAt: Date.UTC(2026, 10, 24),
      daysLeft: 28,
    };
    render(<TrashView items={[item]} language="ru" today="2026-11-25" onRestore={vi.fn()} onPurge={vi.fn()} />);
    const row = screen.getByRole("group", { name: "Реакции" });
    expect(within(row).getAllByRole("img").map((i) => i.textContent)).toEqual([HEART, "🔥"]);
    expect(within(row).queryByRole("button")).toBeNull();
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Восстановить", "Удалить навсегда"]);
  });

  it("строка поиска: чипы под отрывком — подписи; поиск по ним не работает (отрывок — текст записи)", () => {
    const hit: SearchHit = {
      message: mockMessage({ id: "s1", bodyMd: "экзамен сдан", reactions: [chip("🔥")], reactionKeysHad: ["🔥"] }),
      chatTitle: "",
      chatIcon: "📥",
      chatIsInbox: true,
      chatColor: "grey",
      snippet: "экзамен сдан",
      terms: ["экзамен"],
    };
    render(
      <SearchResults
        listId="r"
        query="экзамен"
        chats={[]}
        hits={[hit]}
        hasMore={false}
        loading={false}
        searched
        today="2026-11-25"
        language="ru"
        filters={NO_FILTERS}
        onFiltersChange={vi.fn()}
        canScopeChat
        onPickDate={vi.fn()}
        activeIndex={0}
        onActiveChange={vi.fn()}
        onOpenChat={vi.fn()}
        onOpenHit={vi.fn()}
        onLoadMore={vi.fn()}
      />,
    );
    const option = screen.getByRole("option");
    expect(option.querySelector(".reaction-chip")).toHaveTextContent("🔥");
    expect(option.querySelector("button.reaction-chip")).toBeNull();
  });
});
