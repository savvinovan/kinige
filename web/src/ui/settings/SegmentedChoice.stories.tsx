import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import { SegmentedChoice } from "./SegmentedChoice";
import "./settings-stories.css";

type Size = "normal" | "large";

const OPTIONS = {
  ru: [
    { value: "normal", label: "Обычный" },
    { value: "large", label: "Крупный" },
  ],
  long: [
    { value: "normal", label: "Очень длинное название первого варианта" },
    { value: "large", label: "Second option with a long name too" },
  ],
} as const;

function Demo({ initial, options, focus = false }: { initial: Size; options: keyof typeof OPTIONS; focus?: boolean }) {
  const [value, setValue] = useState<Size>(initial);
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focus) host.current?.querySelector<HTMLButtonElement>('[aria-checked="true"]')?.focus();
  }, [focus]);
  return (
    <div className="story-settings-part" ref={host}>
      <div className="story-settings-part__inner">
        <SegmentedChoice<Size> label="Размер текста" options={OPTIONS[options]} value={value} onChange={setValue} />
      </div>
    </div>
  );
}

const meta = {
  title: "Компоненты/Выбор из двух",
  component: Demo,
  parameters: { layout: "fullscreen" },
  args: { initial: "normal", options: "ru" },
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** "Обычный" is selected. */
export const Обычный: Story = {};

/** "Крупный" is selected. */
export const Крупный: Story = { args: { initial: "large" } };

/** Long labels, Cyrillic and Latin: the segments grow with the text. */
export const ДлинныеПодписи: Story = { args: { options: "long" } };

/** Keyboard focus on the selected option; the arrows move focus and selection. */
export const Фокус: Story = { args: { focus: true } };
