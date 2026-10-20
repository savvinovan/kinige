import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Frame } from "../stories/Frame";
import { Composer, type ComposerButtons } from "./Composer";

interface DemoProps {
  initial?: string;
  buttons?: ComposerButtons;
  error?: string;
  autoFocus?: boolean;
}

/** Поле у нижнего края окна с рабочим состоянием (черновик живёт в истории). */
function Demo({ initial = "", buttons, error, autoFocus }: DemoProps) {
  const [value, setValue] = useState(initial);
  return (
    <Frame bottom>
      <Composer value={value} onChange={setValue} onSubmit={() => setValue("")} buttons={buttons} error={error} autoFocus={autoFocus} />
    </Frame>
  );
}

const meta = {
  title: "Компоненты/Поле ввода",
  component: Demo,
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Пустое: подсказка «Написать заметку…», слева «файл», справа «эмодзи» и «микрофон». */
export const Пусто: Story = {};

/** Черновик: несколько строк, поле выросло; кириллица и латиница. */
export const Черновик: Story = {
  args: {
    initial:
      "Нужно начать готовиться к системному дизайну.\nПосмотреть материалы по Raft и distributed transactions.\n\n- [ ] прочитать главу про консенсус",
  },
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
