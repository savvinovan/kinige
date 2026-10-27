import type { EmojiData, EmojiDataResolver } from "frimousse";
import type { ReactNode } from "react";
import { EmojiResolverProvider } from "../ui/emoji/EmojiResolver";

// Только для тестов. jsdom не раскладывает элементы: у области прокрутки пикера нет размеров, а frimousse по ним решает,
// какие строки рисовать (виртуальный список), и следит за размерами через ResizeObserver, которого в jsdom нет. Заглушка
// подставляет размеры: ячейка 40, область 40 x 8 ячеек. Она позволяет проверять обвязку (поиск, «Недавние», выбор,
// клавиатуру), но не раскладку: раскладку и вид показывают истории Storybook.

const CELL = 40;

/** Ставит заглушки; возвращает функцию, которая возвращает всё как было. */
export function installEmojiLayoutStub(): () => void {
  const clientHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientHeight");
  const offsetWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth");
  const scrollTo = HTMLElement.prototype.scrollTo;
  const observer = globalThis.ResizeObserver;

  class StubResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  globalThis.ResizeObserver = StubResizeObserver as unknown as typeof ResizeObserver;
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get(this: HTMLElement) {
      return this.hasAttribute("frimousse-viewport") ? CELL * 8 : CELL;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "offsetWidth", { configurable: true, get: () => CELL * 9 });
  // frimousse прокручивает область к активной ячейке; в jsdom прокрутки нет.
  HTMLElement.prototype.scrollTo = () => {};

  return () => {
    globalThis.ResizeObserver = observer;
    if (clientHeight) Object.defineProperty(HTMLElement.prototype, "clientHeight", clientHeight);
    if (offsetWidth) Object.defineProperty(HTMLElement.prototype, "offsetWidth", offsetWidth);
    HTMLElement.prototype.scrollTo = scrollTo;
  };
}

/** Данные из двух категорий для тестов пикера. */
export const STUB_EMOJI_DATA: EmojiData = {
  locale: "ru",
  categories: [
    { index: 0, label: "Смайлики" },
    { index: 3, label: "Животные" },
  ],
  skinTones: { dark: "", light: "", medium: "", "medium-dark": "", "medium-light": "" },
  emojis: [
    { emoji: "\u{1F600}", label: "Широко улыбается", category: 0, version: 1, tags: ["smile", "улыбка"] },
    { emoji: "\u{1F622}", label: "Плачет", category: 0, version: 1, tags: ["cry", "слёзы"] },
    { emoji: "\u{1F436}", label: "Морда собаки", category: 3, version: 1, tags: ["dog", "пёс"] },
    { emoji: "\u{1F431}", label: "Морда кошки", category: 3, version: 1, tags: ["cat", "кот"] },
  ],
};

/** Резолвер с этими данными. Одна и та же функция на всех рендерах: frimousse кэширует данные по её идентичности. */
export const STUB_EMOJI_RESOLVER: EmojiDataResolver = async () => STUB_EMOJI_DATA;

/** Пикеры внутри получают маленький набор вместо emojibase-data (быстро, без динамического import). */
export function StubEmojiData({ resolver = STUB_EMOJI_RESOLVER, children }: { resolver?: EmojiDataResolver; children: ReactNode }) {
  return <EmojiResolverProvider value={resolver}>{children}</EmojiResolverProvider>;
}
