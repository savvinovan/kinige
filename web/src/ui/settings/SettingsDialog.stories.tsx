import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState } from "react";
import type { MirrorStatus } from "../../api/generated/MirrorStatus";
import type { Settings } from "../../api/generated/Settings";
import type { TextSize } from "../../api/generated/TextSize";
import { SettingsDialog } from "./SettingsDialog";
import "./settings-stories.css";

const mirror = (patch: Partial<MirrorStatus> = {}): MirrorStatus => ({
  folder: "~/Documents/Kinige",
  isDefault: true,
  error: null,
  pendingDays: 0,
  ...patch,
});
const settings = (patch: Partial<MirrorStatus> = {}, textSize: TextSize = "normal"): Settings => ({ textSize, mirror: mirror(patch) });

interface DemoProps {
  settings: Settings;
  choosing?: boolean;
  actionError?: string;
  /** Ставит фокус на кнопку «Выбрать…»: так история показывает кольцо фокуса без мыши. */
  focusChoose?: boolean;
}

/** Окно настроек поверх «главного окна» (заглушка из поверхностей): выбор размера текста работает, остальное — без вызовов. */
function Demo({ settings: initial, choosing, actionError, focusChoose = false }: DemoProps) {
  const [textSize, setTextSize] = useState(initial.textSize);
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Родительский эффект идёт после дочернего: фокус из истории перекрывает начальный фокус окна (кнопка закрытия).
    if (focusChoose) [...(host.current?.querySelectorAll<HTMLButtonElement>(".path-row .button") ?? [])][0]?.focus();
  }, [focusChoose]);
  return (
    <div className="story-settings-window" ref={host}>
      <div className="story-settings-window__sidebar" />
      <div className="story-settings-window__feed">
        <div className="story-settings-window__plate" />
        <div className="story-settings-window__plate story-settings-window__plate--tall" />
        <div className="story-settings-window__plate" />
      </div>
      <SettingsDialog
        settings={{ ...initial, textSize }}
        onClose={() => {}}
        onChooseFolder={() => {}}
        onTextSizeChange={setTextSize}
        choosing={choosing}
        actionError={actionError}
      />
    </div>
  );
}

const meta = {
  title: "Компоненты/Настройки",
  component: Demo,
  parameters: { layout: "fullscreen" },
  args: { settings: settings() },
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

const LONG_PATH =
  "/Users/александр.саввинов/Документы и настройки/Мои документы за все годы/Личный журнал и заметки на каждый день/Kinige-копия-в-формате-Markdown-для-чтения-без-приложения";

/** Папка не выбрана: пустое состояние, «Выбрать…», предупреждение «открытый текст» на месте, статуса копии нет. */
export const ПапкаНеВыбрана: Story = { args: { settings: settings({ folder: null, isDefault: false }) } };

/** Папка по умолчанию и копия актуальна: бейдж «Папка по умолчанию», «Копия актуальна». */
export const ПоУмолчаниюАктуальна: Story = {};

/** Выбрана другая папка, два дня ещё ждут записи: бейджа нет, склонение «2 дня ждут». */
export const ДругаяПапкаЖдётЗаписи: Story = {
  args: { settings: settings({ folder: "/Users/alexandr/Dropbox/Журнал", isDefault: false, pendingDays: 2 }) },
};

/** Папка недоступна (диск отключён): ошибка со значком и цветом ошибки, role="alert"; записи сохранены. */
export const ОшибкаПапкаНедоступна: Story = {
  args: { settings: settings({ folder: "/Volumes/Backup/Kinige", isDefault: false, error: "folder_unavailable", pendingDays: 3 }) },
};

/** Не удалось записать копию: то же оформление, другой текст. */
export const ОшибкаЗаписи: Story = { args: { settings: settings({ error: "write_failed", pendingDays: 1 }) } };

/** Очень длинный путь с кириллицей: переносится по разделителям и внутри длинного имени, вёрстка не ломается. */
export const ДлинныйПуть: Story = { args: { settings: settings({ folder: LONG_PATH, isDefault: false }) } };

/** «Крупный» выбран: сегмент отмечен заливкой и линией акцента. */
export const КрупныйТекстВыбран: Story = { args: { settings: settings({}, "large") } };

/** Системный диалог выбора папки открыт: «Выбрать…» приглушена и объявлена недоступной (aria-disabled). */
export const ВыборПапкиОткрыт: Story = { args: { choosing: true } };

/** Не удалось открыть выбор папки: отдельная ошибка под строкой папки. */
export const ОшибкаВыбора: Story = { args: { actionError: "Не удалось открыть выбор папки. Попробуйте ещё раз." } };

/** Фокус с клавиатуры на «Выбрать…». */
export const ФокусНаВыборе: Story = { args: { focusChoose: true } };

/** Английский: папка по умолчанию, два дня ждут записи. */
export const EnПоУмолчанию: Story = {
  args: { settings: settings({ pendingDays: 2 }) },
  globals: { language: "en" },
};

/** Английский: папка не выбрана. */
export const EnПапкаНеВыбрана: Story = {
  args: { settings: settings({ folder: null, isDefault: false }) },
  globals: { language: "en" },
};

/** Английский: ошибка записи копии. */
export const EnОшибка: Story = {
  args: { settings: settings({ error: "folder_unavailable", isDefault: false, folder: "/Volumes/Backup/Kinige" }) },
  globals: { language: "en" },
};

/** Английский: длинный путь. */
export const EnДлинныйПуть: Story = {
  args: { settings: settings({ folder: LONG_PATH, isDefault: false }, "large") },
  globals: { language: "en" },
};

/** Английский: ошибка выбора папки. */
export const EnОшибкаВыбора: Story = {
  args: { actionError: "Could not open the folder picker. Please try again." },
  globals: { language: "en" },
};
