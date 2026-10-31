import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import type { ShortcutStatus } from "../../api/generated/ShortcutStatus";
import { SettingsGroup } from "../prefs/SettingsGroup";
import { ShortcutRow } from "./ShortcutRow";
import "./settings-stories.css";

interface DemoProps {
  status: ShortcutStatus;
  error?: string;
  /** Сразу включает запись нового сочетания (нажатием «Изменить»): в историях мыши нет. */
  recording?: boolean;
  /** Фокус на кнопке «Изменить»: кольцо фокуса с клавиатуры. */
  focusChange?: boolean;
  /** Узкая колонка (около 34 знаков). */
  narrow?: boolean;
}

/** Строка сочетания на «бумаге» шириной окна настроек; запись работает: нажатое сочетание сохраняется в состоянии истории. */
function Demo({ status: initial, error, recording = false, focusChange = false, narrow = false }: DemoProps) {
  const [status, setStatus] = useState(initial);
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const button = host.current?.querySelector<HTMLButtonElement>(".shortcut-row__actions button");
    if (recording) button?.click();
    else if (focusChange) button?.focus();
  }, [recording, focusChange]);
  return (
    <div className="story-settings-part" ref={host}>
      <div className={`story-settings-part__inner${narrow ? " story-settings-part__inner--narrow" : ""}`}>
        <SettingsGroup>
          <ShortcutRow
            status={status}
            error={error}
            onChange={(accelerator) => setStatus({ ...status, accelerator, isDefault: false, state: "assigned" })}
            onReset={() => setStatus(initial)}
            onRecording={() => undefined}
          />
        </SettingsGroup>
      </div>
    </div>
  );
}

const capture: ShortcutStatus = { kind: "capture", accelerator: "Ctrl+Alt+KeyN", isDefault: true, state: "assigned" };

const meta = {
  title: "Компоненты/Строка сочетания",
  component: Demo,
  parameters: { layout: "fullscreen" },
  args: { status: capture },
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Назначено, по умолчанию: название, капсулы ⌃ ⌥ N, «Изменить»; ничего лишнего. */
export const Назначено: Story = {};

/** Не по умолчанию: все четыре модификатора, рядом «Сбросить». */
export const НеПоУмолчанию: Story = {
  args: { status: { kind: "clipboard", accelerator: "Ctrl+Alt+Shift+Cmd+KeyK", isDefault: false, state: "assigned" } },
};

/** Занято другой программой: капсулы приглушены, строка ошибки со значком; «Сбросить» есть и у умолчания. */
export const Занято: Story = { args: { status: { ...capture, state: "taken" } } };

/** Запись нового сочетания: вместо капсул поле «Нажмите новое сочетание… Esc — отмена», фокус в нём, кнопок нет. */
export const ЗаписьНового: Story = { args: { recording: true } };

/** Ошибка сохранения нового сочетания: под строкой, цвет и значок ошибки. */
export const ОшибкаСохранения: Story = {
  args: {
    status: { kind: "clipboard", accelerator: "Alt+Shift+KeyV", isDefault: true, state: "assigned" },
    error: "Это сочетание уже назначено другому действию.",
  },
};

/** Фокус с клавиатуры на «Изменить». */
export const Фокус: Story = { args: { focusChange: true } };

/** Длинные подписи (английский, занято и не по умолчанию) в узкой колонке: капсулы и кнопки переносятся, ничего не вылезает. */
export const EnДлинныеПодписи: Story = {
  args: { status: { kind: "clipboard", accelerator: "Ctrl+Alt+Shift+Cmd+F12", isDefault: false, state: "taken" }, narrow: true },
  globals: { language: "en" },
};

/** Узкая колонка, русский: поле записи с длинной подсказкой переносится. */
export const УзкаяЗапись: Story = {
  args: { status: { kind: "clipboard", accelerator: "Ctrl+Alt+Shift+Cmd+F12", isDefault: false, state: "assigned" }, narrow: true, recording: true },
};

/** Английский: запись нового сочетания. */
export const EnЗаписьНового: Story = { args: { recording: true }, globals: { language: "en" } };

/** Английский: назначено. */
export const EnНазначено: Story = { globals: { language: "en" } };

/** Пунктуация и пробел на капсулах. */
export const ПунктуацияИПробел: Story = { args: { status: { ...capture, accelerator: "Alt+Backquote", isDefault: false } } };
