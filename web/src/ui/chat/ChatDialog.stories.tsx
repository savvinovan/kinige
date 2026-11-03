import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef } from "react";
import { useLanguage } from "../../i18n/I18n";
import { EmojiStoryData } from "../emoji/EmojiStoryData";
import { ChatDialog, type ChatDialogProps } from "./ChatDialog";
import "./chat-stories.css";

interface DemoProps extends Partial<Pick<ChatDialogProps, "mode" | "initialName" | "initialEmoji" | "error" | "busy">> {
  /** The name is typed into the field after display (as from the keyboard). */
  typed?: string;
  /** The picker is opened by the emoji button after display. */
  openPicker?: boolean;
  /** After the picker opens, the emoji with this number is chosen from "Recent" (like a click). */
  pickRecent?: number;
}

const RECENT = ["📚", "💡", "🍳", "🧭", "😀"];

function typeInto(input: HTMLInputElement, text: string) {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, text);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

/** The chat window over the "main window" (a stub made of surfaces): the window is pinned to the top and does not shift when the picker opens. */
function Demo({ mode = "create", initialName, initialEmoji, error, busy, typed, openPicker = false, pickRecent }: DemoProps) {
  const language = useLanguage();
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // The parent's effect runs after the window's effects (it puts focus in the field): the story's actions go on top.
    const root = host.current;
    const input = root?.querySelector<HTMLInputElement>('.chat-dialog__name input');
    if (typed && input) typeInto(input, typed);
    if (openPicker) root?.querySelector<HTMLButtonElement>(".chat-dialog__emoji")?.click();
  }, [typed, openPicker]);
  // The picker appears in the document after a re-render caused by the click: "Recent" is awaited via an observer.
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

/** New chat, empty: the "Name" field is focused, 💬 is muted (a hint), "Create" is disabled. */
export const Создание: Story = {};

/** The name is entered: "Create" is enabled. */
export const СоздатьСИменем: Story = { args: { typed: "Книги" } };

/** The picker is open under the field: the window grew down, the heading stayed in place. */
export const СоздатьСПикером: Story = { args: { typed: "Книги", openPicker: true } };

/** An emoji is chosen from "Recent": the picker closed, the button shows 📚, focus returned to the field. */
export const СоздатьЭмодзиВыбран: Story = { args: { typed: "Книги", openPicker: true, pickRecent: 0 } };

/** A long name of Cyrillic and Latin: the field does not grow, the text scrolls inside. */
export const СоздатьДлинноеИмя: Story = {
  args: { typed: "Подготовка к собеседованию по системному дизайну and distributed systems, часть вторая и третья" },
};

/** A Rust error under the field (role=alert): the field has an error border, the entered text stays. */
export const СоздатьОшибка: Story = { args: { typed: "x", error: "Введите название чата." } };

/** A request is in flight: the fields and buttons are disabled. */
export const СоздатьЗанято: Story = { args: { typed: "Книги", busy: true } };

/** Rename: the name is filled in, there is no emoji button, "Save". */
export const Переименование: Story = { args: { mode: "rename", initialName: "Книги", initialEmoji: "📚" } };

/** Rename, error. */
export const ПереименованиеОшибка: Story = {
  args: { mode: "rename", initialName: "Книги", error: "Название чата — не длиннее 128 знаков." },
};

/** Change the emoji: the picker only, a choice submits at once; focus in the search. */
export const СменаЭмодзи: Story = { args: { mode: "icon", initialName: "Книги", initialEmoji: "📚" } };

/** Change the emoji, an error above the picker. */
export const СменаЭмодзиОшибка: Story = {
  args: { mode: "icon", initialName: "Книги", error: "Этот эмодзи не подошёл. Выберите другой." },
};

/** English interface. */
export const СозданиеАнглийский: Story = { args: { typed: "Books", openPicker: true }, globals: { language: "en" } };

/** English, rename. */
export const ПереименованиеАнглийский: Story = { args: { mode: "rename", initialName: "Books" }, globals: { language: "en" } };

/** Large text: the window, field, picker and buttons grow with the text. */
export const СозданиеКрупныйТекст: Story = { args: { typed: "Книги", openPicker: true }, globals: { textScale: "large" } };

/** Large text, rename. */
export const ПереименованиеКрупныйТекст: Story = { args: { mode: "rename", initialName: "Книги" }, globals: { textScale: "large" } };
