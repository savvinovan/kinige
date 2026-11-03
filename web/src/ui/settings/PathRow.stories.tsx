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

/** The folder row on "paper", at the width of the settings window. */
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

/** The default folder: the path and a badge. */
export const ПоУмолчанию: Story = {};

/** Another folder: no badge. */
export const ДругаяПапка: Story = { args: { path: "/Users/alexandr/Dropbox/Журнал", isDefault: false } };

/** No folder is chosen. */
export const НеВыбрана: Story = { args: { path: null, isDefault: false } };

/** A long path with Cyrillic and Latin: it wraps, the button stays on the right. */
export const ДлинныйПуть: Story = {
  args: {
    path: "/Users/александр.саввинов/Документы и настройки/Мои документы за все годы/Личный журнал/Kinige-копия-в-формате-Markdown-для-чтения-без-приложения-Kinige",
    isDefault: false,
  },
};

/** The folder picker dialog is open: the button is unavailable. */
export const Занято: Story = { args: { busy: true } };

/** Keyboard focus on "Выбрать…". */
export const Фокус: Story = { args: { focus: true } };

/** English. */
export const EnНеВыбрана: Story = { args: { path: null, isDefault: false }, globals: { language: "en" } };
