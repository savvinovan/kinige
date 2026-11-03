import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import { useLanguage } from "../../i18n/I18n";
import { EmojiStoryData } from "../emoji/EmojiStoryData";
import { Frame } from "../stories/Frame";
import { Composer, type ComposerButtons } from "./Composer";

interface DemoProps {
  initial?: string;
  buttons?: ComposerButtons;
  error?: string;
  autoFocus?: boolean;
  /** Buttons are only drawn ("Coming later"); in D1 this is so everywhere. */
  viewOnly?: boolean;
  /** The emoji picker is connected: the "emoji" button is live (slice 3). */
  emoji?: boolean;
  /** The picker panel is open after display (as after a click on the button). */
  openPicker?: boolean;
}

/** A field at the bottom edge of the window with working state (the draft lives in the story). */
function Demo({ initial = "", buttons, error, autoFocus, viewOnly = true, emoji = false, openPicker = false }: DemoProps) {
  const [value, setValue] = useState(initial);
  const language = useLanguage();
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (openPicker) host.current?.querySelector<HTMLButtonElement>('button[aria-haspopup="dialog"]')?.click();
  }, [openPicker]);
  return (
    <EmojiStoryData wait={openPicker}>
      <Frame bottom>
        <div ref={host}>
          <Composer
            value={value}
            onChange={setValue}
            onSubmit={() => setValue("")}
            buttons={buttons}
            error={error}
            autoFocus={autoFocus}
            viewOnly={viewOnly}
            emoji={emoji ? { recent: ["📚", "💡", "🍳", "🧭"], language } : undefined}
          />
        </div>
      </Frame>
    </EmojiStoryData>
  );
}

const meta = {
  title: "Компоненты/Поле ввода",
  component: Demo,
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Empty: the "Write a note..." hint, "file" on the left, "emoji" and "microphone" on the right (in D1 they are only drawn). */
export const Пусто: Story = {};

/** Buttons with a function (how it will be when their slices are ready): ordinary, respond to hover. */
export const КнопкиРаботают: Story = { args: { viewOnly: false } };

/** Draft: several lines, the field has grown; Cyrillic and Latin. */
export const Черновик: Story = {
  args: {
    initial:
      "Нужно начать готовиться к системному дизайну.\nПосмотреть материалы по Raft и distributed transactions.\n\n- [ ] прочитать главу про консенсус",
  },
};

/** A very long word without spaces: wraps by characters, no horizontal scrolling. */
export const ДлинноеСлово: Story = {
  args: { initial: `ссылка:\n${"https://example.org/".repeat(5)}${"я".repeat(700)}` },
};

/** Very long text: the field grows up to 40% of the window height, then it scrolls inside. */
export const ДлинныйТекст: Story = {
  args: {
    initial: Array.from({ length: 30 }, (_, i) => `Строка ${i + 1}: пишешь как в мессенджере, перечитываешь как книгу.`).join("\n"),
  },
};

/** A long line without spaces wraps inside the field instead of widening it. */
export const ДлиннаяСтрокаБезПробелов: Story = {
  args: {
    initial: "Сверхъестественнонепроизносимоесловобезединогопробелаинизкогомногобуквенногообрыва_и_ещё_одно_очень_длинное_слово_без_пробелов",
  },
};

/** Send error: the message above the field, the draft in place, a border in the error color. */
export const Ошибка: Story = {
  args: { initial: "Купить билеты на субботу", error: "Не удалось отправить. Текст сохранён в черновике." },
};

/** Buttons by slice: in the app a button appears when its function works (here only "emoji"). */
export const ТолькоЭмодзи: Story = {
  args: { buttons: { emoji: true } },
};

/** No buttons. */
export const БезКнопок: Story = {
  args: { buttons: {} },
};

/** Keyboard focus in the field: a ring around the whole field. */
export const Фокус: Story = {
  args: { autoFocus: true },
};

/** With text: the "microphone" was replaced by a round accent "Send" button (the same as Enter). */
export const СТекстом: Story = { args: { initial: "Купить билеты на субботу" } };

/** Keyboard focus on the "Send" button: the focus ring around the round button. */
export const ФокусНаОтправить: Story = {
  args: { initial: "Купить билеты на субботу" },
  render: (args) => <FocusedSend {...args} />,
};

/** The picker is connected: the "emoji" button is live (in D1 it is only drawn), "Coming later" is removed from it. */
export const СЭмодзи: Story = { args: { emoji: true, viewOnly: false } };

/** The picker panel above the "emoji" button: recent, categories, search; the chosen emoji goes where the caret is. */
export const ПикерОткрыт: Story = { args: { emoji: true, viewOnly: false, openPicker: true, initial: "Купить билеты на субботу" } };

/** The picker panel, English interface. */
export const ПикерОткрытАнглийский: Story = {
  args: { emoji: true, viewOnly: false, openPicker: true, initial: "Buy tickets for Saturday" },
  globals: { language: "en" },
};

/** The picker panel, large text. */
export const ПикерОткрытКрупныйТекст: Story = {
  args: { emoji: true, viewOnly: false, openPicker: true, initial: "Купить билеты на субботу" },
  globals: { textScale: "large" },
};

/** Puts focus on "Send" after display: this is how the story shows the focus ring without a mouse. */
function FocusedSend(args: DemoProps) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector<HTMLButtonElement>('button[aria-label="Отправить"]')?.focus(), []);
  return (
    <div ref={host}>
      <Demo {...args} />
    </div>
  );
}
