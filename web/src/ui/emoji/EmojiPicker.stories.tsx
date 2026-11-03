import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef } from "react";
import type { Language } from "../../i18n";
import { EmojiPicker } from "./EmojiPicker";
import { EmojiStoryData, LOADING_RESOLVER } from "./EmojiStoryData";
import { Frame } from "../stories/Frame";

const RECENT = ["📚", "💡", "🍳", "🧭", "😀", "🎧", "🌲", "✅"];
const MANY_RECENT = Array.from({ length: 30 }, (_, i) => String.fromCodePoint(0x1f600 + i));

interface DemoProps {
  recent?: readonly string[];
  language?: Language;
  /** The query is typed into the search field after display: this is how the story shows search results without a keyboard. */
  query?: string;
  /** Down and right arrow in the search: highlights a cell from the keyboard, as when choosing with the arrows. */
  keyboard?: boolean;
  loading?: boolean;
  narrow?: boolean;
}

/** Sets the field value so that React sees the input (it does not notice a plain value assignment). */
function typeInto(input: HTMLInputElement, text: string) {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, text);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

/** The picker on a plate, as it looks in the popover panel above the input field. */
function Demo({ recent = RECENT, language = "ru", query, keyboard = false, loading = false, narrow = false }: DemoProps) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const input = host.current?.querySelector<HTMLInputElement>('input[type="search"]');
    if (!input) return;
    if (query) typeInto(input, query);
    if (keyboard) {
      // The cells appear after the data loads; the down arrow highlights the first one, right the second.
      const press = (key: string) => input.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
      const timer = window.setInterval(() => {
        if (!host.current?.querySelector('[role="gridcell"]:not([aria-label=""])')) return;
        window.clearInterval(timer);
        input.focus();
        press("ArrowDown");
        press("ArrowRight");
      }, 50);
      return () => window.clearInterval(timer);
    }
  }, [query, keyboard]);
  return (
    <EmojiStoryData wait={!loading} resolver={loading ? LOADING_RESOLVER : undefined}>
      <Frame surface="paper">
        <div className={narrow ? "story-emoji-panel story-emoji-panel--narrow" : "story-emoji-panel"} ref={host}>
          <EmojiPicker recent={recent} language={language} onPick={() => {}} />
        </div>
      </Frame>
    </EmojiStoryData>
  );
}

const meta = {
  title: "Компоненты/Пикер эмодзи",
  component: Demo,
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Normal: search, "Recent", categories with sticky headings, the grid. The emojibase data comes from the bundle, no network. */
export const Обычное: Story = {};

/** No recent: there is no section, the grid takes the whole panel, the panel height is the same. */
export const БезНедавних: Story = { args: { recent: [] } };

/** More than 24 recent: the first 24 are shown, no repeats. */
export const МногоНедавних: Story = { args: { recent: MANY_RECENT } };

/** Search in Russian: "собака" finds the emoji by name; "Recent" is hidden during the search. */
export const ПоискПоРусски: Story = { args: { query: "собака" } };

/** Search in English with the Russian interface: "dog" finds the same emoji (tags of both languages). */
export const ПоискПоАнглийски: Story = { args: { query: "dog" } };

/** Nothing found: a quiet line in the center. */
export const НичегоНеНайдено: Story = { args: { query: "ъъъъъ" } };

/** The data is still loading: a quiet "Loading emoji..." line, search and "Recent" are already available. */
export const Загрузка: Story = { args: { loading: true } };

/** Keyboard choice: focus stays in the search, the cell is highlighted by background and a focus ring (arrows over the grid, Enter chooses). */
export const ВыборСтрелками: Story = { args: { keyboard: true } };

/** A narrow panel: still 9 columns, the cells are narrower, search and headings do not break. */
export const УзкаяПанель: Story = { args: { narrow: true } };

/** English interface: emoji and category labels in English, search finds Russian words too. */
export const Английский: Story = { args: { language: "en", query: "собака" }, globals: { language: "en" } };

/** Large text: the cells, headings and field grow with the text. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** Large text and the English interface. */
export const КрупныйТекстАнглийский: Story = { args: { language: "en" }, globals: { textScale: "large", language: "en" } };
