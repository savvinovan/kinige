import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import type { ShortcutStatus } from "../../api/generated/ShortcutStatus";
import { SettingsGroup } from "../prefs/SettingsGroup";
import { ShortcutRow } from "./ShortcutRow";
import "./settings-stories.css";

interface DemoProps {
  status: ShortcutStatus;
  error?: string;
  /** Immediately starts recording a new shortcut (by pressing "Изменить"): there is no mouse in stories. */
  recording?: boolean;
  /** Focus on the "Изменить" button: the keyboard focus ring. */
  focusChange?: boolean;
  /** A narrow column (about 34 characters). */
  narrow?: boolean;
}

/** A shortcut row on "paper" at the width of the settings window; recording works: the pressed shortcut is saved in the story state. */
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

/** Assigned, the default: the name, the capsules ⌃ ⌥ N, "Изменить"; nothing extra. */
export const Назначено: Story = {};

/** Not the default: all four modifiers, "Сбросить" next to it. */
export const НеПоУмолчанию: Story = {
  args: { status: { kind: "clipboard", accelerator: "Ctrl+Alt+Shift+Cmd+KeyK", isDefault: false, state: "assigned" } },
};

/** Taken by another program: the capsules are muted, an error row with an icon; "Сбросить" is there even for the default. */
export const Занято: Story = { args: { status: { ...capture, state: "taken" } } };

/** Recording a new shortcut: instead of the capsules the field "Нажмите новое сочетание… Esc — отмена", focus in it, no buttons. */
export const ЗаписьНового: Story = { args: { recording: true } };

/** A save error of the new shortcut: under the row, the error color and icon. */
export const ОшибкаСохранения: Story = {
  args: {
    status: { kind: "clipboard", accelerator: "Alt+Shift+KeyV", isDefault: true, state: "assigned" },
    error: "Это сочетание уже назначено другому действию.",
  },
};

/** Keyboard focus on "Изменить". */
export const Фокус: Story = { args: { focusChange: true } };

/** Long labels (English, taken and not the default) in a narrow column: the capsules and buttons wrap, nothing overflows. */
export const EnДлинныеПодписи: Story = {
  args: { status: { kind: "clipboard", accelerator: "Ctrl+Alt+Shift+Cmd+F12", isDefault: false, state: "taken" }, narrow: true },
  globals: { language: "en" },
};

/** A narrow column, Russian: the recording field with a long hint wraps. */
export const УзкаяЗапись: Story = {
  args: { status: { kind: "clipboard", accelerator: "Ctrl+Alt+Shift+Cmd+F12", isDefault: false, state: "assigned" }, narrow: true, recording: true },
};

/** English: recording a new shortcut. */
export const EnЗаписьНового: Story = { args: { recording: true }, globals: { language: "en" } };

/** English: assigned. */
export const EnНазначено: Story = { globals: { language: "en" } };

/** Punctuation and Space on the capsules. */
export const ПунктуацияИПробел: Story = { args: { status: { ...capture, accelerator: "Alt+Backquote", isDefault: false } } };
