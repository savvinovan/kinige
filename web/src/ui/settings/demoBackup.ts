import type { BackupStatus } from "../../api/generated/BackupStatus";

// Копия базы для историй и тестов. Время местное и фиксированное (27.10.2026): метки «сегодня» и «вчера» стабильны.
export const BACKUP_NOW = new Date(2026, 9, 27, 17, 0);

export const BACKUP_FOLDER = "~/Library/Application Support/Kinige/backups";

export const DEFAULT_BACKUP: BackupStatus = {
  folder: BACKUP_FOLDER,
  lastAt: new Date(2026, 9, 27, 9, 12).getTime(),
  count: 7,
  error: null,
};
