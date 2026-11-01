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

/** Запись на месте: «3 года спустя · 31 октября 2023» и начало старой записи, одной строкой; щелчок открывает её. */
export const Живая: Story = {};

/** Старая запись в корзине: «Запись в корзине», цитата не нажимается. */
export const ЗаписьВКорзине: Story = { args: { quote: { ...LIVE, state: "trash" } } };

/** Старая запись удалена навсегда: «Исходная запись удалена», сам ответ остаётся. */
export const Удалена: Story = {
  args: { quote: { ...LIVE, state: "missing", chatId: null, journalDate: null, excerpt: "", yearsLater: 0 } },
};

/** Разница меньше года (ответы, импортированные из Telegram): подписи «N лет спустя» нет. */
export const МеньшеГода: Story = { args: { quote: { ...LIVE, journalDate: "2023-03-14", yearsLater: 0, excerpt: "Едем в субботу?" } } };

/** Ответ на чужую реплику из импорта: в цитате имя автора и начало реплики. */
export const ЧужаяРеплика: Story = {
  args: { quote: { ...LIVE, journalDate: "2023-03-14", yearsLater: 0, excerpt: "Анна К. Ну что, едем в субботу? Я уже взяла билеты" } },
};

/** Очень длинное начало записи обрезается многоточием; плашка не шире 65 знаков. */
export const ДлинноеНачало: Story = {
  args: { quote: { ...LIVE, excerpt: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации." } },
};

/** Английский. */
export const Английский: Story = { globals: { language: "en" } };

/** Крупный текст. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

/** Все состояния в колонке (английский: «3 years later · October 31, 2023»). */
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
