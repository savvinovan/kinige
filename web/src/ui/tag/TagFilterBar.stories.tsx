import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef } from "react";
import { Frame, FrameLabel } from "../stories/Frame";
import { TagFilterBar } from "./TagFilterBar";

const meta = {
  title: "Компоненты/Фильтр по тегу",
  component: TagFilterBar,
  args: { tag: "идея", onClear: () => undefined },
  decorators: [(Story) => <Frame bottom><Story /></Frame>],
} satisfies Meta<typeof TagFilterBar>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Обычное: «Показаны записи с #идея» тихо, тег акцентным цветом, справа «×». */
export const Обычное: Story = {};

/** Латиница, цифры, подчёркивание. */
export const Латиница: Story = { args: { tag: "reading_list_2026" } };

/** Длинный тег переносится внутри колонки, кнопка «×» остаётся справа. */
export const ДлинныйТег: Story = {
  args: { tag: "очень_длинный_тег_про_всё_что_угодно_и_ещё_немного_about_everything_else_in_the_world" },
};

/** Английский. */
export const Английский: Story = { args: { tag: "ideas" }, globals: { language: "en" } };

/** Крупный текст. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** Узкая колонка (проверка переноса). */
export const Узкая: Story = {
  render: (args) => (
    <>
      <FrameLabel>Колонка около 36 знаков</FrameLabel>
      <div style={{ maxWidth: "var(--layout-measure-column)" }}>
        <TagFilterBar {...args} tag="длинный_тег_для_узкой_колонки" />
      </div>
    </>
  ),
};

/** Фокус с клавиатуры на «×»: кольцо фокуса. */
function FocusedClear(args: React.ComponentProps<typeof TagFilterBar>) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector("button")?.focus(), []);
  return (
    <div ref={host}>
      <TagFilterBar {...args} />
    </div>
  );
}
export const Фокус: Story = { render: (args) => <FocusedClear {...args} /> };
