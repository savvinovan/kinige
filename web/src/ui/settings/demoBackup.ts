import type { BackupStatus } from "../../api/generated/BackupStatus";

// The database copy for stories and tests. The time is local and fixed (27.10.2026): the "сегодня" and "вчера" marks are stable.
export const BACKUP_NOW = new Date(2026, 9, 27, 17, 0);

export const BACKUP_FOLDER = "~/Library/Application Support/Kinige/backups";

export const DEFAULT_BACKUP: BackupStatus = {
  folder: BACKUP_FOLDER,
  lastAt: new Date(2026, 9, 27, 9, 12).getTime(),
  count: 7,
  error: null,
};
