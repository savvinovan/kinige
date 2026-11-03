import type { Meta, StoryObj } from "@storybook/react-vite";
import type { BackupStatus } from "../../api/generated/BackupStatus";
import { BackupRow } from "./BackupRow";
import { BACKUP_FOLDER, BACKUP_NOW, DEFAULT_BACKUP } from "./demoBackup";
import "./settings-stories.css";

/** The database file copy status row on "paper", at the width of the settings window. "Now" is fixed (27.10.2026, 17:00). */
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

/** The copy was taken today, there are seven of them. */
export const Сегодня: Story = { args: { backup: DEFAULT_BACKUP } };

/** The copy was taken yesterday. */
export const Вчера: Story = { args: { backup: { ...DEFAULT_BACKUP, lastAt: new Date(2026, 9, 26, 22, 30).getTime(), count: 3 } } };

/** There are no copies yet. */
export const ЕщёНеСнималась: Story = { args: { backup: { ...DEFAULT_BACKUP, lastAt: null, count: 0 } } };

/** The copies folder is unavailable. */
export const ПапкаНедоступна: Story = { args: { backup: { ...DEFAULT_BACKUP, error: "folder_unavailable" } } };

/** The copy could not be written. */
export const ОшибкаЗаписи: Story = { args: { backup: { ...DEFAULT_BACKUP, error: "write_failed" } } };

/** The integrity check found corruption: a prominent warning, the day's copy is not taken. */
export const Повреждение: Story = { args: { backup: { ...DEFAULT_BACKUP, error: "integrity_failed" } } };

/** Auto-deleting a corrupted entry from the trash did not work. */
export const АвтоудалениеНеСработало: Story = { args: { backup: { ...DEFAULT_BACKUP, error: "purge_failed" } } };

/** A long path of the copies folder wraps. */
export const ДлинныйПуть: Story = {
  args: { backup: { ...DEFAULT_BACKUP, folder: `${BACKUP_FOLDER}/очень/длинный/путь/${"вложенная-папка/".repeat(6)}backups` } },
};

/** English. */
export const English: Story = { args: { backup: { ...DEFAULT_BACKUP, error: "integrity_failed" } }, globals: { language: "en" } };
