import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactionView } from "../../api/generated/ReactionView";
import { installEmojiLayoutStub, StubEmojiData } from "../../test/emoji";
import { act, fireEvent, render, screen, within } from "../../test/render";
import { Message } from "../message/Message";
import { ReactionChoicesProvider } from "./ReactionsContext";

// Slice 5a in the bubble: the chips under the text, the reaction buttons of the hover bar, the limits, the order of the quick emoji and the picker.

const HEART = "❤\u{FE0F}";
const sentAt = new Date(2026, 10, 18, 9, 5);
const toolbar = () => screen.getByRole("toolbar", { name: "Действия с сообщением" });
const barButtons = () => [...toolbar().querySelectorAll("button")];

let restoreLayout: () => void;
beforeAll(() => {
  restoreLayout = installEmojiLayoutStub();
});
afterAll(() => restoreLayout());
// "Now" is pinned to 25 November 2026, so "today" in a chip's tooltip never depends on the day the test runs.
beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(Date.UTC(2026, 10, 25, 12, 0)));
});
afterEach(() => vi.useRealTimers());

const chip = (emoji: string, putAt = Date.now(), yearsLater = 0): ReactionView => ({ emoji, putAt, putTz: "UTC", yearsLater });
/** `count` different emoji that are none of the quick ones (the picture frames and weather of U+1F300). */
const emoji = (count: number) => Array.from({ length: count }, (_, i) => String.fromCodePoint(0x1f300 + i));

interface Setup {
  reactions?: ReactionView[];
  keysHad?: string[];
  quick?: string[];
  recent?: string[];
  readOnly?: boolean;
  onReact?: ((emoji: string, on: boolean) => void) | null;
  showActions?: boolean;
  mode?: "view" | "deleted";
}

function view({ reactions = [], keysHad = [], quick = [HEART, "😂", "😢"], recent = [], readOnly = false, onReact, showActions = true, mode }: Setup) {
  const react = onReact ?? undefined;
  return (
    <StubEmojiData>
      <ReactionChoicesProvider value={{ quick, recent }}>
        <Message
          markdown="Сдал экзамен!"
          sentAt={sentAt}
          timeZone="UTC"
          reactions={reactions}
          reactionKeysHad={keysHad}
          readOnly={readOnly}
          onReact={react}
          showActions={showActions}
          mode={mode}
        />
      </ReactionChoicesProvider>
    </StubEmojiData>
  );
}

function setup(options: Setup = {}) {
  const onReact = options.onReact === null ? null : (options.onReact ?? vi.fn());
  const utils = render(view({ ...options, onReact }));
  return { onReact, ...utils, again: (next: Setup) => utils.rerender(view({ ...next, onReact })) };
}

describe("ряд реакций под текстом", () => {
  it("запись без реакций выглядит как раньше: ни ряда, ни класса, у текста остаётся место для времени", () => {
    const { container } = setup();
    expect(container.querySelector(".reaction-row")).toBeNull();
    expect(container.querySelector(".message")).not.toHaveClass("message--reacted");
    expect(container.querySelector(".message__spacer")).not.toBeNull();
  });

  it("чипы идут в порядке постановки внутри плашки, время в конце их ряда; у текста остаётся место под время, чтобы плашка не сужалась", () => {
    const { container } = setup({ reactions: [chip(HEART), chip("🔥")] });
    const row = within(container.querySelector<HTMLElement>(".message")!).getByRole("group", { name: "Реакции" });
    expect(within(row).getAllByRole("button").map((b) => b.textContent)).toEqual([HEART, "🔥"]);
    expect(container.querySelector(".message")).toHaveClass("message--reacted");
    expect(container.querySelector(".message__spacer")).not.toBeNull();
    expect(container.querySelector(".message--time-below")).toBeNull();
    expect(container.querySelector(".message__time")).toBeInTheDocument();
  });

  it("щелчок по чипу снимает реакцию, как в Telegram", () => {
    const { onReact } = setup({ reactions: [chip(HEART), chip("🔥")] });
    fireEvent.click(within(screen.getByRole("group", { name: "Реакции" })).getAllByRole("button")[1] as HTMLElement);
    expect(onReact).toHaveBeenCalledExactlyOnceWith("🔥", false);
  });

  it("подсказка и имя для диктора: когда поставлена; у поздней реакции «N г.» и «год спустя»", () => {
    const putAt = Date.UTC(2026, 10, 19, 12, 0);
    setup({ reactions: [chip("🥲", putAt, 1)] });
    const button = screen.getByRole("button", { name: /^🥲, Поставлена 19 ноября 2026 · 1 год спустя\. Снять$/ });
    expect(button).toHaveAttribute("title", "Поставлена 19 ноября 2026 · 1 год спустя");
    expect(button).toHaveTextContent("1 г.");
  });

  it("сегодняшняя реакция: «Поставлена сегодня в ЧЧ:ММ»", () => {
    setup({ reactions: [chip("🔥", Date.UTC(2026, 10, 25, 9, 5))] });
    expect(screen.getByRole("button", { name: "🔥, Поставлена сегодня в 09:05. Снять" })).toBeInTheDocument();
  });

  it("без обработчика, у записи только для чтения и у удалённой чипы — подписи без роли кнопки", () => {
    for (const options of [{ onReact: null }, { readOnly: true }, { mode: "deleted" as const }]) {
      const { unmount } = setup({ reactions: [chip(HEART)], ...options });
      const row = screen.getByRole("group", { name: "Реакции" });
      expect(within(row).queryByRole("button")).toBeNull();
      expect(within(row).getByRole("img", { name: /^❤️, / })).toBeInTheDocument();
      unmount();
    }
  });
});

describe("кнопки реакций в ряду действий", () => {
  it("три быстрых эмодзи, «Поставить реакцию», тонкий разделитель и затем прежние пять кнопок", () => {
    setup();
    expect(barButtons().map((b) => b.getAttribute("aria-label"))).toEqual([
      `Реакция ${HEART}`,
      "Реакция 😂",
      "Реакция 😢",
      "Поставить реакцию",
      "Копировать",
      "Редактировать",
      "Сделать задачей",
      "В избранное",
      "Ещё",
    ]);
    expect(toolbar().querySelector(".message-actions__divider")).toBeInTheDocument();
  });

  it("без обработчика реакций ряд прежний: пять кнопок", () => {
    setup({ onReact: null });
    expect(barButtons()).toHaveLength(5);
  });

  it("нажатая кнопка (aria-pressed) — живая реакция; щелчок переключает: ставит или снимает", () => {
    const { onReact } = setup({ reactions: [chip(HEART)] });
    const [heart, laugh] = barButtons() as [HTMLButtonElement, HTMLButtonElement];
    expect(heart).toHaveAttribute("aria-pressed", "true");
    expect(laugh).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(laugh);
    expect(onReact).toHaveBeenLastCalledWith("😂", true);
    fireEvent.click(heart);
    expect(onReact).toHaveBeenLastCalledWith(HEART, false);
  });

  it("сердце с селектором вариации и без него — одна реакция", () => {
    setup({ reactions: [chip("❤")] });
    expect(barButtons()[0]).toHaveAttribute("aria-pressed", "true");
  });

  it("клавиатура: стрелки идут по всему ряду, начиная с эмодзи; один Tab-стоп", () => {
    setup();
    const buttons = barButtons();
    expect(buttons.filter((b) => b.tabIndex === 0)).toHaveLength(1);
    buttons[0]?.focus();
    fireEvent.keyDown(buttons[0] as HTMLElement, { key: "ArrowRight" });
    expect(buttons[1]).toHaveFocus();
    fireEvent.keyDown(buttons[1] as HTMLElement, { key: "End" });
    expect(buttons.at(-1)).toHaveFocus();
    fireEvent.keyDown(buttons.at(-1) as HTMLElement, { key: "ArrowRight" });
    expect(buttons[0]).toHaveFocus();
  });

  it("десять живых реакций: кнопки, которые добавили бы новую, aria-disabled с причиной; нажатые ещё снимаются", () => {
    const ten = emoji(9).map((e) => chip(e));
    const { onReact } = setup({ reactions: [...ten, chip(HEART)], keysHad: [...emoji(9), "❤"] });
    const [heart, laugh, sad, add] = barButtons() as HTMLButtonElement[];
    for (const blocked of [laugh, sad, add]) {
      expect(blocked).toHaveAttribute("aria-disabled", "true");
      expect(blocked?.getAttribute("title")).toContain("На записи не больше 10 реакций");
      fireEvent.click(blocked as HTMLElement);
    }
    expect(onReact).not.toHaveBeenCalled();
    expect(heart).toHaveAttribute("aria-pressed", "true");
    expect(heart).not.toHaveAttribute("aria-disabled");
    fireEvent.click(heart as HTMLElement);
    expect(onReact).toHaveBeenCalledExactlyOnceWith(HEART, false);
  });

  it("32 разных за всю жизнь: новое эмодзи нельзя, прежние (даже снятые) ставятся; причина названа", () => {
    // 32 different keys: the heart and the laughing face are among them, the crying face is not.
    const keys = [...emoji(30), "❤", "😂"];
    expect(new Set(keys).size).toBe(32);
    const { onReact } = setup({ keysHad: keys });
    const [heart, laugh, sad] = barButtons() as HTMLButtonElement[];
    expect(heart).not.toHaveAttribute("aria-disabled");
    expect(laugh).not.toHaveAttribute("aria-disabled");
    expect(sad).toHaveAttribute("aria-disabled", "true");
    expect(sad?.getAttribute("title")).toContain("На этой записи уже были 32 разные реакции");
    fireEvent.click(sad as HTMLElement);
    fireEvent.click(laugh as HTMLElement);
    expect(onReact).toHaveBeenCalledExactlyOnceWith("😂", true);
  });

  it("запись только для чтения: кнопки реакций приглушены с объяснением, ничего не вызывают", () => {
    const { onReact } = setup({ readOnly: true });
    for (const button of barButtons().slice(0, 4)) {
      expect(button).toHaveAttribute("aria-disabled", "true");
      fireEvent.click(button);
    }
    expect(onReact).not.toHaveBeenCalled();
  });
});

describe("порядок быстрых эмодзи", () => {
  const labels = () => barButtons().slice(0, 3).map((b) => b.textContent);

  it("берётся, когда ряд появился, и не меняется, пока указатель на записи: следующая кнопка не уезжает из-под указателя", () => {
    const { again, onReact } = setup({ quick: ["🔥", HEART, "😂"] });
    expect(labels()).toEqual(["🔥", HEART, "😂"]);
    fireEvent.click(barButtons()[2] as HTMLElement);
    expect(onReact).toHaveBeenCalledWith("😂", true);
    // After the click Rust reports 😂 as the latest: the new order does not apply while the bar is shown.
    again({ quick: ["😂", "🔥", HEART], showActions: true });
    expect(labels()).toEqual(["🔥", HEART, "😂"]);
    // The bar went away (the pointer left): the next time it appears it has the new order.
    again({ quick: ["😂", "🔥", HEART], showActions: false });
    expect(labels()).toEqual(["😂", "🔥", HEART]);
  });
});

describe("окно выбора реакции", () => {
  it("«Поставить реакцию» открывает выбор с недавними реакциями; выбор ставит и закрывает; фокус возвращается на кнопку, даже если щелчок её не сфокусировал", async () => {
    const { onReact } = setup({ recent: ["🎉", "🙏"] });
    const add = screen.getByRole("button", { name: "Поставить реакцию" });
    expect(document.activeElement).not.toBe(add);
    fireEvent.click(add);
    const dialog = await screen.findByRole("dialog", { name: "Выбор реакции" });
    expect(dialog.parentElement).toBe(document.body);
    const recent = within(dialog).getByRole("region", { name: "Недавние" });
    fireEvent.click(within(recent).getByRole("button", { name: "🙏" }));
    expect(onReact).toHaveBeenCalledExactlyOnceWith("🙏", true);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(add).toHaveFocus();
  });

  it("Esc закрывает выбор, ничего не ставит и возвращает фокус на кнопку; щелчок мимо закрывает без возврата", async () => {
    const { onReact } = setup({ recent: ["🎉"] });
    const add = screen.getByRole("button", { name: "Поставить реакцию" });
    fireEvent.click(add);
    fireEvent.keyDown(await screen.findByRole("searchbox"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(add).toHaveFocus();
    (document.activeElement as HTMLElement).blur();
    fireEvent.click(add);
    await screen.findByRole("dialog");
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(add).not.toHaveFocus();
    expect(onReact).not.toHaveBeenCalled();
  });

  it("контекстное меню: Shift+F10, «Поставить реакцию…» открывает малое окно с тремя эмодзи и «Ещё»; выбор ставит, фокус возвращается на запись", async () => {
    const { onReact, container } = setup({ showActions: false });
    const plate = container.querySelector<HTMLElement>("article")!;
    plate.focus();
    fireEvent.keyDown(plate, { key: "F10", shiftKey: true });
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Поставить реакцию…" }));
    const dialog = await screen.findByRole("dialog", { name: "Выбор реакции" });
    expect(within(dialog).getAllByRole("button").map((b) => b.textContent)).toEqual([HEART, "😂", "😢", "Ещё"]);
    expect(within(dialog).getAllByRole("button")[0]).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "ArrowRight" });
    expect(within(dialog).getAllByRole("button")[1]).toHaveFocus();
    fireEvent.click(within(dialog).getAllByRole("button")[1] as HTMLElement);
    expect(onReact).toHaveBeenCalledExactlyOnceWith("😂", true);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(plate).toHaveFocus();
  });

  it("«Ещё» превращает малое окно в полный выбор с поиском", async () => {
    const { container } = setup({ showActions: false, recent: ["🎉"] });
    const plate = container.querySelector<HTMLElement>("article")!;
    fireEvent.keyDown(plate, { key: "F10", shiftKey: true });
    fireEvent.click(screen.getByRole("menuitem", { name: "Поставить реакцию…" }));
    const dialog = await screen.findByRole("dialog", { name: "Выбор реакции" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Ещё эмодзи" }));
    expect(await within(dialog).findByRole("searchbox")).toHaveFocus();
    expect(within(dialog).getByRole("region", { name: "Недавние" })).toBeInTheDocument();
  });

  it("Esc в малом окне закрывает его с возвратом фокуса; пункт меню у записи только для чтения недоступен, у записи без обработчика его нет", () => {
    const { container, unmount } = setup({ showActions: false });
    const plate = container.querySelector<HTMLElement>("article")!;
    fireEvent.keyDown(plate, { key: "F10", shiftKey: true });
    fireEvent.click(screen.getByRole("menuitem", { name: "Поставить реакцию…" }));
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(plate).toHaveFocus();
    unmount();

    for (const options of [{ onReact: null }, { readOnly: true }]) {
      const mounted = setup({ showActions: false, ...options });
      const article = mounted.container.querySelector<HTMLElement>("article")!;
      fireEvent.keyDown(article, { key: "F10", shiftKey: true });
      expect(screen.queryByRole("menuitem", { name: "Поставить реакцию…" })).toBeNull();
      act(() => mounted.unmount());
    }
  });
});
