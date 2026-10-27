import type { Meta, StoryObj } from "@storybook/react-vite";
import type { BackupInfo } from "../../api/generated/BackupInfo";
import { RecoveryScreen } from "./RecoveryScreen";

// Время копий — местное, как задано здесь: снимки не зависят от пояса машины.
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

/** Три копии (копия дня, перед обновлением, копия дня): самая новая выбрана, кнопка называет её дату и время. */
export const ТриКопии: Story = {};

/** Одна копия. */
export const ОднаКопия: Story = { args: { backups: THREE.slice(0, 1) } };

/** Копий нет: тихое сообщение и подсказка, кнопки нет. */
export const БезКопий: Story = { args: { backups: [] } };

/** Много копий: список прокручивается внутри, заголовок и кнопка остаются на месте. */
export const МногоКопий: Story = { args: { backups: MANY } };

/** Идёт восстановление: кнопка приглушена, выбор заблокирован. */
export const Восстановление: Story = { args: { busy: true } };

/** Ошибка восстановления: строка `role="alert"` перед кнопкой. */
export const Ошибка: Story = { args: { error: "Копия не найдена или не открывается." } };

/** Проверка нашла повреждение в работающем приложении: пояснение и «Перезапустить», списка нет. */
export const Работающее: Story = { args: { mode: "running", backups: [] } };

/** То же, перезапуск идёт и ошибка. */
export const РаботающееОшибка: Story = { args: { mode: "running", backups: [], busy: true, error: "Что-то пошло не так." } };

/** Английский. */
export const Английский: Story = { args: { language: "en" }, globals: { language: "en" } };

/** Английский, режим «работающее». */
export const РаботающееEn: Story = { args: { mode: "running", backups: [], language: "en" }, globals: { language: "en" } };

/** Крупный текст. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** Папка Markdown-копии будет переименована при восстановлении: тихое пояснение над кнопкой. */
export const СПапкойКопии: Story = { args: { mirrorFolder: "/Users/alexandr/Documents/Kinige" } };

/** Папка Markdown-копии будет переименована при восстановлении: тихая строка под пояснением. */
export const СПапкойКопииEn: Story = {
  args: { mirrorFolder: "/Users/alexandr/Documents/Kinige", language: "en" },
  globals: { language: "en", textScale: "large" },
};
