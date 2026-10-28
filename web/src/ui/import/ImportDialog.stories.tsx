import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ImportPreview } from "../../api/generated/ImportPreview";
import type { ImportReport } from "../../api/generated/ImportReport";
import { ImportDialog } from "./ImportDialog";

// Окно «Импорт из Telegram» по шагам: файл → чаты → прогресс → отчёт; ошибки, пустой список, длинные имена, en, крупный текст.
const noop = () => undefined;

const PREVIEW: ImportPreview = {
  chats: [
    { id: "1000001", name: null, kind: "saved", total: 1682, existing: 0 },
    { id: "2000002", name: "Пётр Тестов", kind: "personal", total: 240, existing: 40 },
    { id: "2000003", name: "Анна", kind: "personal", total: 12, existing: 12 },
    { id: "2000004", name: "Пустой чат", kind: "personal", total: 0, existing: 0 },
  ],
  skippedChats: 7,
};

const REPORT: ImportReport = {
  imported: 1675,
  skippedExisting: 0,
  skippedService: 2,
  skippedEmpty: 5,
  mediaPlaceholders: 785,
  chatsCreated: 0,
  backupName: "pre-import-20261028T125000Z.db",
  stopped: false,
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

/** Шаг 1: короткая инструкция и кнопка выбора файла. */
export const Файл: Story = {};

/** Файл открывается: кнопка недоступна. */
export const ФайлОткрывается: Story = { args: { opening: true } };

/** Не тот файл: строка ошибки D1, ничего не записано. */
export const НеТотФайл: Story = { args: { error: "import_not_telegram" } };

/** Файл больше предела. */
export const СлишкомБольшой: Story = { args: { error: "import_file_too_large" } };

/** Шаг 2: «Избранное» отмечено, личные чаты нет; «уже есть»; чат без записей недоступен; группы и каналы пропущены. */
export const Чаты: Story = { args: { step: "choose", preview: PREVIEW } };

/** Отмечены все чаты; у «Анны» всё уже импортировано. */
export const ЧатыОтмечены: Story = {
  args: { step: "choose", preview: PREVIEW, selected: new Set(["1000001", "2000002", "2000003"]) },
};

/** Ни один чат не отмечен: «Импортировать» недоступна. */
export const НичегоНеОтмечено: Story = { args: { step: "choose", preview: PREVIEW, selected: new Set() } };

/** Длинные имена обрезаются многоточием, их много — список прокручивается внутри окна. */
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
      })),
    },
  },
};

/** В файле нет чатов, которые можно импортировать. */
export const НетЧатов: Story = { args: { step: "choose", preview: { chats: [], skippedChats: 4 } } };

/** Ошибка запуска: копия базы не снялась, ничего не записано. */
export const ОшибкаЗапуска: Story = { args: { step: "choose", preview: PREVIEW, error: "import_backup_failed" } };

/** Шаг 3: прогресс «3 200 из 15 000»; крестика нет, только «Остановить». */
export const Прогресс: Story = { args: { step: "running", progress: { done: 3200, total: 15000 } } };

/** Остановка запрошена: кнопка недоступна, пока Rust дописывает пачку. */
export const Останавливаем: Story = { args: { step: "running", progress: { done: 3200, total: 15000 }, stopping: true } };

/** Шаг 4: отчёт тихими строками, называет файл копии базы. */
export const Отчёт: Story = { args: { step: "report", report: REPORT } };

/** Повторный импорт того же файла: новых 0, «пропущено, уже импортировано». */
export const ОтчётПовтор: Story = {
  args: {
    step: "report",
    report: { ...REPORT, imported: 0, skippedExisting: 15000, skippedService: 0, skippedEmpty: 0, mediaPlaceholders: 0, backupName: null },
  },
};

/** Импорт остановлен: в отчёте сказано, что остаток доимпортируется. */
export const ОтчётОстановлен: Story = { args: { step: "report", report: { ...REPORT, imported: 3200, stopped: true } } };

/** Английский. */
export const ЧатыEn: Story = { args: { step: "choose", preview: PREVIEW }, globals: { language: "en" } };

/** Английский, отчёт. */
export const ОтчётEn: Story = { args: { step: "report", report: REPORT }, globals: { language: "en" } };

/** Крупный текст: шаг чатов. */
export const ЧатыКрупный: Story = { args: { step: "choose", preview: PREVIEW }, globals: { textScale: "large" } };

/** Крупный текст, английский, прогресс. */
export const ПрогрессEnКрупный: Story = {
  args: { step: "running", progress: { done: 3200, total: 15000 } },
  globals: { language: "en", textScale: "large" },
};
