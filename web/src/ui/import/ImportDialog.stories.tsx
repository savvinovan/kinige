import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ImportPreview } from "../../api/generated/ImportPreview";
import type { ImportReport } from "../../api/generated/ImportReport";
import { NO_MEDIA } from "../../api/mockData";
import { ImportDialog } from "./ImportDialog";

// The "Import from Telegram" window step by step: file -> chats -> progress -> report; errors, an empty list, long names, en, large text.
const noop = () => undefined;

const PREVIEW: ImportPreview = {
  chats: [
    { id: "1000001", name: null, kind: "saved", total: 1682, existing: 0, ...NO_MEDIA },
    { id: "2000002", name: "Пётр Тестов", kind: "personal", total: 240, existing: 40, ...NO_MEDIA },
    { id: "2000003", name: "Анна", kind: "personal", total: 12, existing: 12, ...NO_MEDIA },
    { id: "2000004", name: "Пустой чат", kind: "personal", total: 0, existing: 0, ...NO_MEDIA },
  ],
  skippedChats: 7,
};

const REPORT: ImportReport = {
  imported: 1675,
  skippedExisting: 0,
  skippedService: 2,
  skippedEmpty: 5,
  mediaPlaceholders: 785,
  mediaAttached: 0,
  mediaBytes: 0,
  mediaMissing: 0,
  mediaSkipped: 0,
  diskFull: false,
  chatsCreated: 0,
  backupName: "pre-import-20261028T125000Z.db",
  stopped: false,
};

/** An export with media (slice 7): 1,240 files, 6.7 GB, 12 of them not in the export. */
const MEDIA_PREVIEW: ImportPreview = {
  chats: [{ id: "1000001", name: null, kind: "saved", total: 1682, existing: 0, mediaFiles: 1240, mediaBytes: 7_194_000_000, mediaMissing: 12 }],
  skippedChats: 0,
};

const meta = {
  title: "Экраны/Импорт из Telegram",
  component: ImportDialog,
  parameters: { layout: "fullscreen" },
  args: {
    step: "pick",
    selected: new Set(["1000001"]),
    onPick: noop,
    onToggle: noop,
    onRun: noop,
    onStop: noop,
    onClose: noop,
  },
} satisfies Meta<typeof ImportDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Step 1: a short instruction and the file picker button. */
export const Файл: Story = {};

/** The file is opening: the button is unavailable. */
export const ФайлОткрывается: Story = { args: { opening: true } };

/** The wrong file: the D1 error line, nothing was written. */
export const НеТотФайл: Story = { args: { error: "import_not_telegram" } };

/** The file is over the limit. */
export const СлишкомБольшой: Story = { args: { error: "import_file_too_large" } };

/** Step 2: "Saved Messages" is checked, personal chats are not; "already there"; a chat without entries is unavailable; groups and channels are skipped. */
export const Чаты: Story = { args: { step: "choose", preview: PREVIEW } };

/** All chats are checked; everything of "Анна" is already imported. */
export const ЧатыОтмечены: Story = {
  args: { step: "choose", preview: PREVIEW, selected: new Set(["1000001", "2000002", "2000003"]) },
};

/** No chat is checked: "Import" is unavailable. */
export const НичегоНеОтмечено: Story = { args: { step: "choose", preview: PREVIEW, selected: new Set() } };

/** Long names are cut with an ellipsis, and there are many of them: the list scrolls inside the window. */
export const ДлинныеИмена: Story = {
  args: {
    step: "choose",
    selected: new Set(["x0"]),
    preview: {
      skippedChats: 0,
      chats: Array.from({ length: 24 }, (_, i) => ({
        id: `x${i}`,
        name: `Очень длинное имя собеседника номер ${i + 1}, которое не помещается в строку окна импорта and Some English Words`,
        kind: "personal" as const,
        total: 1000 + i,
        existing: i % 3 === 0 ? i : 0,
        ...NO_MEDIA,
      })),
    },
  },
};

/** The file has no chats that can be imported. */
export const НетЧатов: Story = { args: { step: "choose", preview: { chats: [], skippedChats: 4 } } };

/** A start error: the database backup was not made, nothing was written. */
export const ОшибкаЗапуска: Story = { args: { step: "choose", preview: PREVIEW, error: "import_backup_failed" } };

/** Step 3: progress "3 200 из 15 000"; no cross, only "Stop". */
export const Прогресс: Story = { args: { step: "running", progress: { done: 3200, total: 15000, bytesDone: 0, bytesTotal: 0 } } };

/** A stop was requested: the button is unavailable while Rust finishes the batch. */
export const Останавливаем: Story = {
  args: { step: "running", progress: { done: 3200, total: 15000, bytesDone: 0, bytesTotal: 0 }, stopping: true },
};

/** Media (slice 7): the choose step names the files and their size, "not in the export" included. */
export const ЧатыСМедиа: Story = {
  args: { step: "choose", preview: MEDIA_PREVIEW, selected: new Set(["1000001"]) },
};

/** Media: progress runs in gigabytes. */
export const ПрогрессСМедиа: Story = {
  args: { step: "running", progress: { done: 900, total: 1682, bytesDone: 3_900_000_000, bytesTotal: 7_194_000_000 } },
};

/** Media: the report says how many files were attached, how big they are and how many were not in the export. */
export const ОтчётСМедиа: Story = {
  args: {
    step: "report",
    report: { ...REPORT, mediaPlaceholders: 40, mediaAttached: 1228, mediaBytes: 7_100_000_000, mediaMissing: 12, mediaSkipped: 1 },
  },
};

/** The disk filled up: the batch was rolled back, the report says so. */
export const ОтчётДиск: Story = {
  args: { step: "report", report: { ...REPORT, mediaAttached: 600, mediaBytes: 3_000_000_000, stopped: false, diskFull: true } },
};

/** Step 4: the report in quiet lines, names the database backup file. */
export const Отчёт: Story = { args: { step: "report", report: REPORT } };

/** A repeat import of the same file: 0 new, "skipped, already imported". */
export const ОтчётПовтор: Story = {
  args: {
    step: "report",
    report: { ...REPORT, imported: 0, skippedExisting: 15000, skippedService: 0, skippedEmpty: 0, mediaPlaceholders: 0, backupName: null },
  },
};

/** The import was stopped: the report says the remainder will be imported later. */
export const ОтчётОстановлен: Story = { args: { step: "report", report: { ...REPORT, imported: 3200, stopped: true } } };

/** English. */
export const ЧатыEn: Story = { args: { step: "choose", preview: PREVIEW }, globals: { language: "en" } };

/** English, the report. */
export const ОтчётEn: Story = { args: { step: "report", report: REPORT }, globals: { language: "en" } };

/** Large text: the chats step. */
export const ЧатыКрупный: Story = { args: { step: "choose", preview: PREVIEW }, globals: { textScale: "large" } };

/** Large text, English, progress. */
export const ПрогрессEnКрупный: Story = {
  args: { step: "running", progress: { done: 3200, total: 15000, bytesDone: 0, bytesTotal: 0 } },
  globals: { language: "en", textScale: "large" },
};
