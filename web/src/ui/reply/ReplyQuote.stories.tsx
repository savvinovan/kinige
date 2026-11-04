import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import "../feed/feed.css";
import { Message } from "../message/Message";
import { Frame, FrameLabel } from "../stories/Frame";
import { ReplyQuote } from "./ReplyQuote";

const noop = () => undefined;
const at = (hour: number, minute: number) => new Date(2026, 9, 31, hour, minute);

const LIVE: ReplyTo = {
  id: "old",
  chatId: "c",
  journalDate: "2023-10-31",
  excerpt: "Так будет лучше: перестать откладывать и написать первую страницу",
  state: "live",
  yearsLater: 3,
  attachments: { photos: 0, files: 0, name: null },
};

const meta = {
  title: "Компоненты/Цитата ответа",
  component: ReplyQuote,
  args: { quote: LIVE, onOpen: noop },
  decorators: [(Story) => <Frame><Story /></Frame>],
  render: (args) => (
    <div className="story-column">
      <Message markdown="До сих пор так думаю" sentAt={at(9, 12)} quote={<ReplyQuote {...args} />} />
    </div>
  ),
} satisfies Meta<typeof ReplyQuote>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The entry is in place: "3 years later · 31 October 2023" and the start of the old entry, on one line; a click opens it. */
export const Живая: Story = {};

/** The old entry is in the trash: "Entry in the trash", the quote is not clickable. */
export const ЗаписьВКорзине: Story = { args: { quote: { ...LIVE, state: "trash" } } };

/** The old entry was deleted for good: "Original entry deleted", the reply itself stays. */
export const Удалена: Story = {
  args: { quote: { ...LIVE, state: "missing", chatId: null, journalDate: null, excerpt: "", yearsLater: 0 } },
};

/** The difference is less than a year (replies imported from Telegram): there is no "N years later" label. */
export const МеньшеГода: Story = { args: { quote: { ...LIVE, journalDate: "2023-03-14", yearsLater: 0, excerpt: "Едем в субботу?" } } };

/** A reply to someone else's line from an import: the quote has the author's name and the start of the line. */
export const ЧужаяРеплика: Story = {
  args: { quote: { ...LIVE, journalDate: "2023-03-14", yearsLater: 0, excerpt: "Анна К. Ну что, едем в субботу? Я уже взяла билеты" } },
};

/** A very long start of the entry is cut with an ellipsis; the bubble is no wider than 65 characters. */
export const ДлинноеНачало: Story = {
  args: { quote: { ...LIVE, excerpt: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации." } },
};

/** English. */
export const Английский: Story = { globals: { language: "en" } };

/** Large text. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** All states in a column (English: "3 years later · October 31, 2023"). */
export const ВсеСостояния: Story = {
  render: () => (
    <div className="story-column">
      <FrameLabel>Живая</FrameLabel>
      <Message markdown="Ответ" sentAt={at(9, 12)} quote={<ReplyQuote quote={LIVE} onOpen={noop} />} />
      <FrameLabel>В корзине</FrameLabel>
      <Message markdown="Ответ" sentAt={at(9, 13)} quote={<ReplyQuote quote={{ ...LIVE, state: "trash" }} />} />
      <FrameLabel>Удалена</FrameLabel>
      <Message markdown="Ответ" sentAt={at(9, 14)} quote={<ReplyQuote quote={{ ...LIVE, state: "missing", chatId: null, journalDate: null, excerpt: "", yearsLater: 0 }} />} />
    </div>
  ),
};
