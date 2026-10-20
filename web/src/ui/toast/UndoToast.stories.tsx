import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import { Frame } from "../stories/Frame";
import { UndoToast } from "./UndoToast";

// В историях тост не исчезает сам (десять минут): снимок и просмотр не гонятся с таймером на 5 секунд.
const LONG_MS = 600_000;

function Demo({ message, focus = false }: { message: string; focus?: boolean }) {
  const [open, setOpen] = useState(true);
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focus) host.current?.querySelector("button")?.focus();
  }, [focus]);
  return (
    <Frame bottom>
      <div ref={host}>
        <UndoToast open={open} message={message} onUndo={() => setOpen(false)} onExpire={() => setOpen(false)} durationMs={LONG_MS} />
        {open ? null : <p className="story-frame__label">Тост закрыт: действие отменено или время вышло.</p>}
      </div>
    </Frame>
  );
}

const meta = {
  title: "Компоненты/Тост «Отменить»",
  component: Demo,
  args: { message: "Сообщение удалено" },
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Показан над полем ввода; «Отменить» закрывает его. */
export const Показан: Story = {};

/** Длинный текст и латиница переносятся внутри тоста. */
export const ДлинныйТекст: Story = {
  args: {
    message: "Сообщение «Нужно начать готовиться к системному дизайну и Raft» перемещено в корзину на 30 дней, deleted_at записан",
  },
};

/** Фокус с клавиатуры на «Отменить». */
export const Фокус: Story = { args: { focus: true } };
