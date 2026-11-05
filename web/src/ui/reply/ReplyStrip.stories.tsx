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
  attachments: { photos: 0, files: 0, voices: 0, voiceMs: null, name: null },
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

/** "Reply to the entry of 31 October 2023", the first line of the entry and a cross; the input field is under it. */
export const Обычная: Story = {};

/** The entry went to the trash: the line says so, the reply cannot be sent (the text stays). */
export const ЗаписьВКорзине: Story = { args: { quote: { ...LIVE, state: "trash" } } };

/** The entry was deleted for good. */
export const Удалена: Story = { args: { quote: { ...LIVE, state: "missing", journalDate: null, excerpt: "", yearsLater: 0 } } };

/** The entry being replied to was moved to another chat: Rust will not accept a reply to it, the entry will go as an ordinary one. */
export const ЗаписьПеренесена: Story = { args: { quote: { ...LIVE, state: "moved" } } };

/** A very long first line is cut with an ellipsis. */
export const ДлиннаяСтрока: Story = {
  args: { quote: { ...LIVE, excerpt: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации." } },
};

/** English. */
export const Английский: Story = { globals: { language: "en" } };

/** Large text. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };
