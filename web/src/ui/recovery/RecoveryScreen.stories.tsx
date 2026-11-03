import type { Meta, StoryObj } from "@storybook/react-vite";
import type { BackupInfo } from "../../api/generated/BackupInfo";
import { RecoveryScreen } from "./RecoveryScreen";

// The copy times are local, as set here: screenshots do not depend on the machine's time zone.
const backup = (name: string, kind: BackupInfo["kind"], at: Date): BackupInfo => ({ name, kind, createdAt: at.getTime() });
const THREE: BackupInfo[] = [
  backup("daily-2026-10-27.db", "daily", new Date(2026, 9, 27, 8, 5)),
  backup("pre-migration-2026-10-26.db", "pre_migration", new Date(2026, 9, 26, 18, 30)),
  backup("daily-2026-10-25.db", "daily", new Date(2026, 9, 25, 9, 12)),
];
const MANY: BackupInfo[] = [
  ...THREE,
  ...[24, 23, 22, 21, 20, 19].map((day) => backup(`daily-2026-10-${day}.db`, "daily", new Date(2026, 9, day, 9, 12))),
];

const meta = {
  title: "Экраны/База повреждена",
  component: RecoveryScreen,
  parameters: { layout: "fullscreen" },
  args: { mode: "startup", backups: THREE, language: "ru", onRestore: () => undefined, onRestart: () => undefined },
} satisfies Meta<typeof RecoveryScreen>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Three copies (daily, before an update, daily): the newest is selected, the button names its date and time. */
export const ТриКопии: Story = {};

/** One copy. */
export const ОднаКопия: Story = { args: { backups: THREE.slice(0, 1) } };

/** No copies: a quiet message and a hint, no button. */
export const БезКопий: Story = { args: { backups: [] } };

/** Many copies: the list scrolls inside, the heading and button stay in place. */
export const МногоКопий: Story = { args: { backups: MANY } };

/** A restore is in progress: the button is muted, the choice is blocked. */
export const Восстановление: Story = { args: { busy: true } };

/** A restore error: a `role="alert"` line before the button. */
export const Ошибка: Story = { args: { error: "Копия не найдена или не открывается." } };

/** The check found corruption in a running app: an explanation and "Перезапустить", no list. */
export const Работающее: Story = { args: { mode: "running", backups: [] } };

/** The same, a restart is in progress, and an error. */
export const РаботающееОшибка: Story = { args: { mode: "running", backups: [], busy: true, error: "Что-то пошло не так." } };

/** English. */
export const Английский: Story = { args: { language: "en" }, globals: { language: "en" } };

/** English, the "running" mode. */
export const РаботающееEn: Story = { args: { mode: "running", backups: [], language: "en" }, globals: { language: "en" } };

/** Large text. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** English with large text: a quiet line about the Markdown copy folder under the explanation. */
export const КрупныйТекстEn: Story = { args: { language: "en" }, globals: { language: "en", textScale: "large" } };
