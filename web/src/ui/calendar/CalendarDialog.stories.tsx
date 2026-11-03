import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import "../settings/settings-stories.css";
import { CalendarDialog } from "./CalendarDialog";

/** The calendar over the "main window" (a stub made of surfaces, as for "Settings"): picking a day highlights it and closes without changing anything. */
function Window(props: React.ComponentProps<typeof CalendarDialog>) {
  return (
    <div className="story-settings-window">
      <div className="story-settings-window__sidebar" />
      <div className="story-settings-window__feed">
        <div className="story-settings-window__plate" />
        <div className="story-settings-window__plate story-settings-window__plate--tall" />
      </div>
      <CalendarDialog {...props} />
    </div>
  );
}

const meta = {
  title: "Компоненты/Календарь",
  component: Window,
  parameters: { layout: "fullscreen" },
  args: { value: "2026-10-20", today: "2026-10-27", language: "ru", onPick: () => undefined, onClose: () => undefined },
} satisfies Meta<typeof Window>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Regular: October 2026, the 20th selected, today is the 27th (a dot under the number), the week starts on Monday, days of neighboring months are muted. Focus on the selected day. */
export const Обычное: Story = {};

/** The selected day is today: the fill and the dot together. */
export const ВыбранСегодня: Story = { args: { value: "2026-10-27" } };

/** A past year, a month of six weeks (March 2025: the 1st is a Saturday). */
export const ШестьНедель: Story = { args: { value: "2025-03-14" } };

/** A future entry date: future dates are allowed. */
export const БудущаяДата: Story = { args: { value: "2027-02-28" } };

/** English: the week starts on Sunday, "October 2026", "Today". */
export const Английский: Story = { args: { language: "en" }, globals: { language: "en" } };

/** Large text: the grid grows with the text, the window stays within bounds. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** Error saving the date: a `role="alert"` line under the grid. */
export const Ошибка: Story = { args: { error: "Такой даты нет." } };

/** The date is being saved (busy): picking a day and "Today" do nothing. */
export const Сохраняется: Story = { args: { busy: true } };

/** A live window: the selected day moves on click, months are flipped with the buttons and the keyboard (PageUp / PageDown). */
function Live(args: React.ComponentProps<typeof CalendarDialog>) {
  const [value, setValue] = useState(args.value);
  return <Window {...args} value={value} onPick={setValue} />;
}
export const Живое: Story = { render: (args) => <Live {...args} /> };
