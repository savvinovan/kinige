import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef } from "react";
import { useLanguage } from "../../i18n/I18n";
import { EmojiStoryData } from "../emoji/EmojiStoryData";
import { ChatDialog, type ChatDialogProps } from "./ChatDialog";
import "./chat-stories.css";

interface DemoProps extends Partial<Pick<ChatDialogProps, "mode" | "initialName" | "initialEmoji" | "error" | "busy">> {
  /** Имя вводится в поле после показа (как с клавиатуры). */
  typed?: string;
  /** Пикер раскрыт кнопкой эмодзи после показа. */
  openPicker?: boolean;
  /** После раскрытия пикера выбирается эмодзи из «Недавних» с этим номером (как щелчок). */
  pickRecent?: number;
}

const RECENT = ["📚", "💡", "🍳", "🧭", "😀"];

function typeInto(input: HTMLInputElement, text: string) {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, text);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

/** Окно чата поверх «главного окна» (заглушка из поверхностей): окно прижато к верху и не сдвигается, когда раскрывается пикер. */
function Demo({ mode = "create", initialName, initialEmoji, error, busy, typed, openPicker = false, pickRecent }: DemoProps) {
  const language = useLanguage();
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Эффект родителя выполняется после эффектов окна (оно ставит фокус в поле): действия истории идут поверх.
    const root = host.current;
    const input = root?.querySelector<HTMLInputElement>('.chat-dialog__name input');
    if (typed && input) typeInto(input, typed);
    if (openPicker) root?.querySelector<HTMLButtonElement>(".chat-dialog__emoji")?.click();
  }, [typed, openPicker]);
  // Пикер появляется в документе после перерисовки, вызванной щелчком: «Недавние» ждут по наблюдателю.
  useEffect(() => {
    const root = host.current;
    if (pickRecent === undefined || !root) return;
    const pick = () => {
      const item = root.querySelectorAll<HTMLButtonElement>(".emoji-picker__recent-item")[pickRecent];
      item?.click();
      return item !== undefined;
    };
    if (pick()) return;
    const observer = new MutationObserver(() => {
      if (pick()) observer.disconnect();
    });
    observer.observe(root, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, [pickRecent]);
  return (
    <EmojiStoryData wait={(openPicker && pickRecent === undefined) || mode === "icon"}>
      <div className="story-chat-window" ref={host}>
        <div className="story-chat-window__sidebar" />
        <div className="story-chat-window__feed">
          <div className="story-chat-window__plate" />
          <div className="story-chat-window__plate" />
        </div>
        <ChatDialog
          mode={mode}
          initialName={initialName}
          initialEmoji={initialEmoji}
          recentEmoji={RECENT}
          language={language}
          error={error}
          busy={busy}
          onSubmit={() => {}}
          onClose={() => {}}
        />
      </div>
    </EmojiStoryData>
  );
}

const meta = {
  title: "Компоненты/Окно чата",
  component: Demo,
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Новый чат, пусто: поле «Название» в фокусе, 💬 приглушён (подсказка), «Создать» недоступна. */
export const Создание: Story = {};

/** Имя введено: «Создать» доступна. */
export const СоздатьСИменем: Story = { args: { typed: "Книги" } };

/** Пикер раскрыт под полем: окно выросло вниз, заголовок остался на месте. */
export const СоздатьСПикером: Story = { args: { typed: "Книги", openPicker: true } };

/** Эмодзи выбран из «Недавних»: пикер закрылся, на кнопке 📚, фокус вернулся в поле. */
export const СоздатьЭмодзиВыбран: Story = { args: { typed: "Книги", openPicker: true, pickRecent: 0 } };

/** Длинное имя из кириллицы и латиницы: поле не растёт, текст прокручивается внутри. */
export const СоздатьДлинноеИмя: Story = {
  args: { typed: "Подготовка к собеседованию по системному дизайну and distributed systems, часть вторая и третья" },
};

/** Ошибка Rust под полем (role=alert): поле с рамкой ошибки, введённое остаётся. */
export const СоздатьОшибка: Story = { args: { typed: "x", error: "Введите название чата." } };

/** Запрос идёт: поля и кнопки недоступны. */
export const СоздатьЗанято: Story = { args: { typed: "Книги", busy: true } };

/** Переименование: имя подставлено, кнопки эмодзи нет, «Сохранить». */
export const Переименование: Story = { args: { mode: "rename", initialName: "Книги", initialEmoji: "📚" } };

/** Переименование, ошибка. */
export const ПереименованиеОшибка: Story = {
  args: { mode: "rename", initialName: "Книги", error: "Название чата — не длиннее 128 знаков." },
};

/** Смена эмодзи: только пикер, выбор сразу отправляет; фокус в поиске. */
export const СменаЭмодзи: Story = { args: { mode: "icon", initialName: "Книги", initialEmoji: "📚" } };

/** Смена эмодзи, ошибка над пикером. */
export const СменаЭмодзиОшибка: Story = {
  args: { mode: "icon", initialName: "Книги", error: "Этот эмодзи не подошёл. Выберите другой." },
};

/** Английский интерфейс. */
export const СозданиеАнглийский: Story = { args: { typed: "Books", openPicker: true }, globals: { language: "en" } };

/** Английский, переименование. */
export const ПереименованиеАнглийский: Story = { args: { mode: "rename", initialName: "Books" }, globals: { language: "en" } };

/** Крупный текст: окно, поле, пикер и кнопки растут вместе с текстом. */
export const СозданиеКрупныйТекст: Story = { args: { typed: "Книги", openPicker: true }, globals: { textScale: "large" } };

/** Крупный текст, переименование. */
export const ПереименованиеКрупныйТекст: Story = { args: { mode: "rename", initialName: "Книги" }, globals: { textScale: "large" } };
