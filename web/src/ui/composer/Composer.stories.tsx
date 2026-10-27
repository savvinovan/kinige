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
  /** Кнопки только нарисованы («Появится позже»); в D1 так везде. */
  viewOnly?: boolean;
  /** Пикер эмодзи подключён: кнопка «эмодзи» живая (слайс 3). */
  emoji?: boolean;
  /** Панель пикера открыта после показа (как после щелчка по кнопке). */
  openPicker?: boolean;
}

/** Поле у нижнего края окна с рабочим состоянием (черновик живёт в истории). */
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

/** Пустое: подсказка «Написать заметку…», слева «файл», справа «эмодзи» и «микрофон» (в D1 они только нарисованы). */
export const Пусто: Story = {};

/** Кнопки с функцией (так будет, когда их слайсы готовы): обычные, откликаются на наведение. */
export const КнопкиРаботают: Story = { args: { viewOnly: false } };

/** Черновик: несколько строк, поле выросло; кириллица и латиница. */
export const Черновик: Story = {
  args: {
    initial:
      "Нужно начать готовиться к системному дизайну.\nПосмотреть материалы по Raft и distributed transactions.\n\n- [ ] прочитать главу про консенсус",
  },
};

/** Очень длинное слово без пробелов: переносится по символам, горизонтальной прокрутки нет. */
export const ДлинноеСлово: Story = {
  args: { initial: `ссылка:\n${"https://example.org/".repeat(5)}${"я".repeat(700)}` },
};

/** Очень длинный текст: поле растёт до 40% высоты окна, дальше внутри прокрутка. */
export const ДлинныйТекст: Story = {
  args: {
    initial: Array.from({ length: 30 }, (_, i) => `Строка ${i + 1}: пишешь как в мессенджере, перечитываешь как книгу.`).join("\n"),
  },
};

/** Длинная строка без пробелов переносится внутри поля, а не раздвигает его. */
export const ДлиннаяСтрокаБезПробелов: Story = {
  args: {
    initial: "Сверхъестественнонепроизносимоесловобезединогопробелаинизкогомногобуквенногообрыва_и_ещё_одно_очень_длинное_слово_без_пробелов",
  },
};

/** Ошибка отправки: сообщение над полем, черновик на месте, рамка цвета ошибки. */
export const Ошибка: Story = {
  args: { initial: "Купить билеты на субботу", error: "Не удалось отправить. Текст сохранён в черновике." },
};

/** Кнопки по слайсам: в приложении кнопка появляется, когда её функция работает (здесь только «эмодзи»). */
export const ТолькоЭмодзи: Story = {
  args: { buttons: { emoji: true } },
};

/** Без кнопок. */
export const БезКнопок: Story = {
  args: { buttons: {} },
};

/** Фокус с клавиатуры в поле: кольцо вокруг всего поля. */
export const Фокус: Story = {
  args: { autoFocus: true },
};

/** С текстом: «микрофон» сменился круглой акцентной кнопкой «Отправить» (то же, что Enter). */
export const СТекстом: Story = { args: { initial: "Купить билеты на субботу" } };

/** Фокус с клавиатуры на кнопке «Отправить»: кольцо фокуса вокруг круглой кнопки. */
export const ФокусНаОтправить: Story = {
  args: { initial: "Купить билеты на субботу" },
  render: (args) => <FocusedSend {...args} />,
};

/** Пикер подключён: кнопка «эмодзи» живая (в D1 она только нарисована), «Появится позже» с неё снято. */
export const СЭмодзи: Story = { args: { emoji: true, viewOnly: false } };

/** Панель пикера над кнопкой «эмодзи»: недавние, категории, поиск; выбранный эмодзи встаёт туда, где стоит курсор. */
export const ПикерОткрыт: Story = { args: { emoji: true, viewOnly: false, openPicker: true, initial: "Купить билеты на субботу" } };

/** Панель пикера, английский интерфейс. */
export const ПикерОткрытАнглийский: Story = {
  args: { emoji: true, viewOnly: false, openPicker: true, initial: "Buy tickets for Saturday" },
  globals: { language: "en" },
};

/** Панель пикера, крупный текст. */
export const ПикерОткрытКрупныйТекст: Story = {
  args: { emoji: true, viewOnly: false, openPicker: true, initial: "Купить билеты на субботу" },
  globals: { textScale: "large" },
};

/** Ставит фокус на «Отправить» после показа: так история показывает кольцо фокуса без мыши. */
function FocusedSend(args: DemoProps) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector<HTMLButtonElement>('button[aria-label="Отправить"]')?.focus(), []);
  return (
    <div ref={host}>
      <Demo {...args} />
    </div>
  );
}
