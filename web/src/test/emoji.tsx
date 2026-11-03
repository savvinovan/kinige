import type { EmojiData, EmojiDataResolver } from "frimousse";
import type { ReactNode } from "react";
import { EmojiResolverProvider } from "../ui/emoji/EmojiResolver";

// For tests only. jsdom does not lay out elements: the picker's scroll area has no sizes, and frimousse uses them to decide
// which rows to draw (a virtual list), and watches the sizes through ResizeObserver, which jsdom does not have. The stub
// supplies sizes: a cell is 40, the area is 40 x 8 cells. It lets tests check the wiring (search, "Недавние" (Recent), selection,
// keyboard), but not the layout: the layout and the look are shown by the Storybook stories.

const CELL = 40;

/** Installs the stubs; returns a function that puts everything back as it was. */
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
  // frimousse scrolls the area to the active cell; jsdom has no scrolling.
  HTMLElement.prototype.scrollTo = () => {};

  return () => {
    globalThis.ResizeObserver = observer;
    if (clientHeight) Object.defineProperty(HTMLElement.prototype, "clientHeight", clientHeight);
    if (offsetWidth) Object.defineProperty(HTMLElement.prototype, "offsetWidth", offsetWidth);
    HTMLElement.prototype.scrollTo = scrollTo;
  };
}

/** Data of two categories for the picker tests. */
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

/** A resolver with this data. The same function on all renders: frimousse caches the data by its identity. */
export const STUB_EMOJI_RESOLVER: EmojiDataResolver = async () => STUB_EMOJI_DATA;

/** The pickers inside get a small set instead of emojibase-data (fast, no dynamic import). */
export function StubEmojiData({ resolver = STUB_EMOJI_RESOLVER, children }: { resolver?: EmojiDataResolver; children: ReactNode }) {
  return <EmojiResolverProvider value={resolver}>{children}</EmojiResolverProvider>;
}
