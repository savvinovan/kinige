import type { Meta, StoryObj } from "@storybook/react-vite";
import type { BackupStatus } from "../../api/generated/BackupStatus";
import { BackupRow } from "./BackupRow";
import { BACKUP_FOLDER, BACKUP_NOW, DEFAULT_BACKUP } from "./demoBackup";
import "./settings-stories.css";

/** Строка состояния копии файла базы на «бумаге», шириной окна настроек. «Сейчас» фиксировано (27.10.2026, 17:00). */
function Demo({ backup }: { backup: BackupStatus }) {
  return (
    <div className="story-settings-part">
      <div className="story-settings-part__inner">
        <BackupRow backup={backup} now={BACKUP_NOW} />
      </div>
    </div>
  );
}

const meta = {
  title: "Компоненты/Копия базы",
  component: Demo,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof Demo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Копия снята сегодня, их семь. */
export const Сегодня: Story = { args: { backup: DEFAULT_BACKUP } };

/** Копия снята вчера. */
export const Вчера: Story = { args: { backup: { ...DEFAULT_BACKUP, lastAt: new Date(2026, 9, 26, 22, 30).getTime(), count: 3 } } };

/** Копий ещё нет. */
export const ЕщёНеСнималась: Story = { args: { backup: { ...DEFAULT_BACKUP, lastAt: null, count: 0 } } };

/** Папка копий недоступна. */
export const ПапкаНедоступна: Story = { args: { backup: { ...DEFAULT_BACKUP, error: "folder_unavailable" } } };

/** Копию записать не удалось. */
export const ОшибкаЗаписи: Story = { args: { backup: { ...DEFAULT_BACKUP, error: "write_failed" } } };

/** Проверка целостности нашла повреждение: заметное предупреждение, копия дня не снимается. */
export const Повреждение: Story = { args: { backup: { ...DEFAULT_BACKUP, error: "integrity_failed" } } };

/** Автоудаление повреждённой записи из корзины не сработало. */
export const АвтоудалениеНеСработало: Story = { args: { backup: { ...DEFAULT_BACKUP, error: "purge_failed" } } };

/** Длинный путь папки копий переносится. */
export const ДлинныйПуть: Story = {
  args: { backup: { ...DEFAULT_BACKUP, folder: `${BACKUP_FOLDER}/очень/длинный/путь/${"вложенная-папка/".repeat(6)}backups` } },
};

/** English. */
export const English: Story = { args: { backup: { ...DEFAULT_BACKUP, error: "integrity_failed" } }, globals: { language: "en" } };
