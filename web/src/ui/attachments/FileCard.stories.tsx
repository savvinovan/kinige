import type { Meta, StoryObj } from "@storybook/react-vite";
import { Bubble, withScene } from "./AttachmentsStage";
import { demoFile } from "./demo";
import { FileCard } from "./FileCard";

const noop = () => undefined;
const MEGABYTE = 1024 * 1024;

const PDF = demoFile("pdf", "Договор аренды 2026.pdf", "file", 2.4 * MEGABYTE);
const LONG = demoFile(
  "long",
  "Квартальный отчёт по продажам и расходам, сводная таблица для бухгалтерии, финальная версия после правок (2026).xlsx",
  "file",
  18 * MEGABYTE,
);

const meta = {
  title: "Компоненты/Вложения: файл",
  component: FileCard,
  args: { attachment: PDF, onOpen: noop },
  decorators: [withScene],
  render: (args) => <Bubble attachments={[args.attachment]} text="Подписанный договор" />,
} satisfies Meta<typeof FileCard>;

export default meta;
type Story = StoryObj<typeof meta>;

/** A PDF: the icon, the name, and "PDF · 2.4 MB". The whole card is one button. */
export const Обычная: Story = {};

/** A very long name is cut in the middle: the start ends with an ellipsis, the end with ".xlsx" stays. */
export const ДлинноеИмя: Story = { args: { attachment: LONG } };

/** Video and audio are plain file cards with their own icon (no players in this slice). */
export const ВидеоИАудио: Story = {
  render: () => (
    <Bubble
      attachments={[
        demoFile("mov", "Прогулка у реки.mp4", "video", 148 * MEGABYTE),
        demoFile("snd", "Голосовая заметка.mp3", "audio", 3.1 * MEGABYTE),
      ]}
    />
  ),
};

/** A file without an extension: only the size under the name. */
export const БезРасширения: Story = { args: { attachment: demoFile("bare", "README", "file", 1200) } };

/** The file is missing from the disk: the same card, "File is missing" under the name; the click does nothing. */
export const ФайлаНет: Story = { args: { attachment: demoFile("gone", "Договор аренды 2026.pdf", "file", 2.4 * MEGABYTE, "missing") } };

/** English: "PDF · 2.4 MB" with a point and a short unit. */
export const Английский: Story = {
  globals: { language: "en" },
  args: { attachment: demoFile("en", "Lease agreement 2026.pdf", "file", 2.4 * MEGABYTE) },
};

/** Large text: the card grows with the text, the name is still cut in the middle. */
export const КрупныйТекст: Story = { globals: { textScale: "large" }, args: { attachment: LONG } };

/** The 640 px window. */
export const УзкоеОкно: Story = { parameters: { narrow: true }, args: { attachment: LONG } };
