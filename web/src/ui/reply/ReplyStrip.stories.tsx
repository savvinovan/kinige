import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { Composer } from "../composer/Composer";
import { Frame } from "../stories/Frame";
import { ReplyStrip } from "./ReplyStrip";

const noop = () => undefined;

const LIVE: ReplyTo = {
  id: "old",
  chatId: "c",
  journalDate: "2023-10-31",
  excerpt: "Так будет лучше: перестать откладывать и написать первую страницу",
  state: "live",
  yearsLater: 3,
};

const meta = {
  title: "Компоненты/Строка ответа над полем",
  component: ReplyStrip,
  args: { quote: LIVE, onCancel: noop },
  decorators: [(Story) => <Frame bottom><Story /></Frame>],
  render: (args) => <Composer value="До сих пор так думаю" onChange={noop} onSubmit={noop} buttons={{ emoji: true }} replyStrip={<ReplyStrip {...args} />} />,
} satisfies Meta<typeof ReplyStrip>;

export default meta;
type Story = StoryObj<typeof meta>;

/** «Ответ на запись · 31 октября 2023», первая строка записи и крестик; под ней поле ввода. */
export const Обычная: Story = {};

/** Запись ушла в корзину: строка так и говорит, отправить ответ нельзя (текст остаётся). */
export const ЗаписьВКорзине: Story = { args: { quote: { ...LIVE, state: "trash" } } };

/** Запись удалена навсегда. */
export const Удалена: Story = { args: { quote: { ...LIVE, state: "missing", journalDate: null, excerpt: "", yearsLater: 0 } } };

/** Очень длинная первая строка обрезается многоточием. */
export const ДлиннаяСтрока: Story = {
  args: { quote: { ...LIVE, excerpt: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации." } },
};

/** Английский. */
export const Английский: Story = { globals: { language: "en" } };

/** Крупный текст. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };
