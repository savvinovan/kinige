import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef } from "react";
import { PathRow } from "./PathRow";
import "./settings-stories.css";

interface DemoProps {
  path: string | null;
  isDefault?: boolean;
  busy?: boolean;
  focus?: boolean;
}

/** Строка папки на «бумаге», шириной окна настроек. */
function Demo({ path, isDefault, busy, focus = false }: DemoProps) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focus) host.current?.querySelector<HTMLButtonElement>("button")?.focus();
  }, [focus]);
  return (
    <div className="story-settings-part" ref={host}>
      <div className="story-settings-part__inner">
        <PathRow path={path} isDefault={isDefault} busy={busy} onChoose={() => {}} />
      </div>
    </div>
  );
}

const meta = {
  title: "Компоненты/Строка папки",
  component: Demo,
  parameters: { layout: "fullscreen" },
  args: { path: "~/Documents/Kinige", isDefault: true },
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Папка по умолчанию: путь и бейдж. */
export const ПоУмолчанию: Story = {};

/** Другая папка: бейджа нет. */
export const ДругаяПапка: Story = { args: { path: "/Users/alexandr/Dropbox/Журнал", isDefault: false } };

/** Папка не выбрана. */
export const НеВыбрана: Story = { args: { path: null, isDefault: false } };

/** Длинный путь с кириллицей и латиницей: переносится, кнопка остаётся справа. */
export const ДлинныйПуть: Story = {
  args: {
    path: "/Users/александр.саввинов/Документы и настройки/Мои документы за все годы/Личный журнал/Kinige-копия-в-формате-Markdown-для-чтения-без-приложения-Kinige",
    isDefault: false,
  },
};

/** Диалог выбора папки открыт: кнопка недоступна. */
export const Занято: Story = { args: { busy: true } };

/** Фокус с клавиатуры на «Выбрать…». */
export const Фокус: Story = { args: { focus: true } };

/** Английский. */
export const EnНеВыбрана: Story = { args: { path: null, isDefault: false }, globals: { language: "en" } };
