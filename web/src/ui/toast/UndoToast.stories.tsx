import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import { Frame } from "../stories/Frame";
import { UndoToast } from "./UndoToast";

// The toast overlays and does not shift content: in the stories it stands above the input field on an empty background.
// In the stories the toast does not disappear by itself (ten minutes): a snapshot and viewing do not race the 5-second timer.
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

/** Shown above the input field; "Undo" closes it. */
export const Показан: Story = {};

/** Long text and Latin wrap inside the toast. */
export const ДлинныйТекст: Story = {
  args: {
    message: "Сообщение «Нужно начать готовиться к системному дизайну и Raft» перемещено в корзину на 30 дней, deleted_at записан",
  },
};

/** Keyboard focus on "Undo". */
export const Фокус: Story = { args: { focus: true } };
