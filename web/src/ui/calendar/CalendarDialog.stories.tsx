import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import "../settings/settings-stories.css";
import { CalendarDialog } from "./CalendarDialog";

/** Календарь поверх «главного окна» (заглушка из поверхностей, как у «Настроек»): выбор дня подсвечивает его и закрывает ничего не меняя. */
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

/** Обычное: октябрь 2026, выбрано 20-е, сегодня 27-е (точка под числом), неделя с понедельника, дни соседних месяцев приглушены. Фокус на выбранном дне. */
export const Обычное: Story = {};

/** Выбранный день — сегодня: заливка и точка вместе. */
export const ВыбранСегодня: Story = { args: { value: "2026-10-27" } };

/** Прошлый год, месяц из шести недель (март 2025: 1-е — суббота). */
export const ШестьНедель: Story = { args: { value: "2025-03-14" } };

/** Будущая дата записи: будущие даты разрешены. */
export const БудущаяДата: Story = { args: { value: "2027-02-28" } };

/** Английский: неделя с воскресенья, «October 2026», «Today». */
export const Английский: Story = { args: { language: "en" }, globals: { language: "en" } };

/** Крупный текст: сетка растёт вместе с текстом, окно остаётся в границах. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** Ошибка сохранения даты: строка `role="alert"` под сеткой. */
export const Ошибка: Story = { args: { error: "Такой даты нет." } };

/** Дата сохраняется (busy): выбор дня и «Сегодня» не срабатывают. */
export const Сохраняется: Story = { args: { busy: true } };

/** Живое окно: выбранный день переносится щелчком, месяцы листаются кнопками и клавиатурой (PageUp / PageDown). */
function Live(args: React.ComponentProps<typeof CalendarDialog>) {
  const [value, setValue] = useState(args.value);
  return <Window {...args} value={value} onPick={setValue} />;
}
export const Живое: Story = { render: (args) => <Live {...args} /> };
