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
  /** Запрос вводится в поле поиска после показа: так история показывает результаты поиска без клавиатуры. */
  query?: string;
  /** Стрелка вниз и вправо в поиске: выделяет ячейку с клавиатуры, как при выборе стрелками. */
  keyboard?: boolean;
  loading?: boolean;
  narrow?: boolean;
}

/** Задаёт значение поля так, чтобы React увидел ввод (обычное присваивание value он не замечает). */
function typeInto(input: HTMLInputElement, text: string) {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, text);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

/** Пикер на плашке, как он выглядит во всплывающей панели над полем ввода. */
function Demo({ recent = RECENT, language = "ru", query, keyboard = false, loading = false, narrow = false }: DemoProps) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const input = host.current?.querySelector<HTMLInputElement>('input[type="search"]');
    if (!input) return;
    if (query) typeInto(input, query);
    if (keyboard) {
      // Ячейки появляются после загрузки данных; стрелка вниз выделяет первую, вправо — вторую.
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

/** Обычное: поиск, «Недавние», категории с липкими заголовками, сетка. Данные emojibase из бандла, без сети. */
export const Обычное: Story = {};

/** Без недавних: секции нет, сетка занимает всю панель, высота панели та же. */
export const БезНедавних: Story = { args: { recent: [] } };

/** Недавних больше 24: показываются первые 24, повторов нет. */
export const МногоНедавних: Story = { args: { recent: MANY_RECENT } };

/** Поиск по-русски: «собака» находит эмодзи по названию; «Недавние» на время поиска скрыты. */
export const ПоискПоРусски: Story = { args: { query: "собака" } };

/** Поиск по-английски при русском интерфейсе: «dog» находит те же эмодзи (теги обоих языков). */
export const ПоискПоАнглийски: Story = { args: { query: "dog" } };

/** Ничего не найдено: тихая строка по центру. */
export const НичегоНеНайдено: Story = { args: { query: "ъъъъъ" } };

/** Данные ещё грузятся: тихая строка «Загрузка эмодзи…», поиск и «Недавние» уже доступны. */
export const Загрузка: Story = { args: { loading: true } };

/** Выбор с клавиатуры: фокус остаётся в поиске, ячейка выделена фоном и кольцом фокуса (стрелки по сетке, Enter выбирает). */
export const ВыборСтрелками: Story = { args: { keyboard: true } };

/** Узкая панель: столбцов по-прежнему 9, ячейки уже, поиск и заголовки не ломаются. */
export const УзкаяПанель: Story = { args: { narrow: true } };

/** Английский интерфейс: подписи эмодзи и категорий по-английски, поиск находит и русские слова. */
export const Английский: Story = { args: { language: "en", query: "собака" }, globals: { language: "en" } };

/** Крупный текст: ячейки, заголовки и поле растут вместе с текстом. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** Крупный текст и английский интерфейс. */
export const КрупныйТекстАнглийский: Story = { args: { language: "en" }, globals: { textScale: "large", language: "en" } };
