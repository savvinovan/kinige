import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef } from "react";
import "../feed/feed.css";
import { Frame, FrameLabel } from "../stories/Frame";
import { Message } from "./Message";

// The time in the stories is fixed (local), so that snapshots do not change from run to run.
const at = (hour: number, minute: number) => new Date(2026, 9, 20, hour, minute);

const CODE = "```\nfn main() {\n    let today = Day::new(2026, 10, 20);\n    println!(\"Записей: {}\", today.count());\n}\n```";

const LONG_CODE =
  "```\ncargo build --release --workspace --all-targets --features sync,encryption,import-telegram,search-morphology,markdown-copy --target-dir /var/tmp/kinige-build-output/release-candidate\n```";

const meta = {
  title: "Компоненты/Сообщение",
  component: Message,
  args: { markdown: "Купить хлеб", sentAt: at(9, 12) },
  decorators: [(Story) => <Frame><Story /></Frame>],
} satisfies Meta<typeof Message>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Normal: short text, a short bubble, the time in the bottom right corner. */
export const Обычное: Story = {};

/** Italic, bold, a link and inline code in one paragraph; the bubble fits the text but is no wider than ~80 characters. */
export const Текст: Story = {
  args: {
    markdown:
      "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать. Потом [прочитал про привычки](https://fs.blog/habits) и починил историю командой `git rebase --onto main`.",
  },
};

/** Headings H1-H3: not much larger than the text (compact scale). */
export const Заголовки: Story = {
  render: () => (
    <>
      <Message markdown={"# Заголовок первого уровня\nТекст под ним."} sentAt={at(8, 40)} />
      <Message markdown={"## Заголовок второго уровня\nТекст под ним."} sentAt={at(8, 41)} />
      <Message markdown={"### Планы на неделю\nЗакончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда."} sentAt={at(8, 42)} />
    </>
  ),
};

/** Lists and tasks: ordinary items, a numbered list, checked and unchecked tasks. */
export const СпискиИЗадачи: Story = {
  render: () => (
    <>
      <Message markdown={"### Купить\n- хлеб\n- молоко\n- кофе"} sentAt={at(10, 0)} />
      <Message markdown={"1. Проснуться\n2. Позавтракать\n3. Записать мысль"} sentAt={at(10, 1)} />
      <Message markdown={"### Планы на выходные\n- [x] Поход в горы\n- [ ] Разобрать фотографии\n- [x] Почитать книгу"} sentAt={at(11, 30)} />
    </>
  ),
};

/** Quote: the bar only on the quote, the next paragraph has no bar. */
export const Цитата: Story = {
  args: {
    markdown: "> Лучше сделано, чем идеально.\n\nЗаписал, чтобы перечитать, когда начну полировать пятый день подряд.",
    sentAt: at(21, 5),
  },
};

/** Code: inline and block; the block has its own fill, the time is under the block. */
export const Код: Story = {
  args: {
    markdown: `Починил историю командой \`git rebase --onto main\`, вот что получилось:\n\n${CODE}`,
    sentAt: at(13, 45),
  },
};

/** Code with a long line and a paragraph after it (KINIG-40): the line wraps, there is no scroll bar under the block, and the first line of the paragraph is not covered by anything, even if the OS always shows scroll bars. */
export const КодСДлиннойСтрокой: Story = {
  args: {
    markdown: `Вот команда сборки:\n\n${LONG_CODE}\n\nПосле неё запускаю проверки и жду зелёного результата.`,
    sentAt: at(14, 10),
  },
};

/** Long text: the bubble hits the ~80 character width and wraps lines; the time stays at the end of the last one. */
export const ДлинныйТекст: Story = {
  args: {
    markdown:
      "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации, и записать вопросы, которые остались после чтения. Потом вернуться к этой записи через неделю и проверить, на сколько удалось ответить.",
    sentAt: at(9, 12),
  },
};

/** A long line without spaces and a long line of code: both wrap, there is no scrolling. */
export const ДлиннаяСтрокаБезПробелов: Story = {
  render: () => (
    <>
      <Message
        markdown="Сверхъестественнонепроизносимоесловобезединогопробелаинизкогомногобуквенногообрыва и ссылка https://example.com/очень/длинный/путь/который/не/помещается/в/плашку/ни/при/каком/размере/окна"
        sentAt={at(9, 12)}
      />
      <Message
        markdown={"```\nlet result = very_long_function_name(first_argument, second_argument, third_argument, fourth_argument, fifth_argument);\n```"}
        sentAt={at(9, 13)}
      />
    </>
  ),
};

/** Cyrillic and Latin in one message, digits, punctuation, dashes. */
export const КириллицаИЛатиница: Story = {
  args: {
    markdown:
      "Съешь же ещё этих мягких французских булок, да выпей чаю. The quick brown fox jumps over the lazy dog. 0123456789 — «ёлочки», “quotes”, ёЁ.",
  },
};

/** States: highlighted, not sent, both together. A series (a smaller gap) is shown by the "Feed" story. */
export const Состояния: Story = {
  render: () => (
    <>
      <FrameLabel>Подсвечено: другая заливка (светлее и бирюзовее плашки)</FrameLabel>
      <Message markdown="Нашлось поиском: системный дизайн" sentAt={at(9, 12)} highlighted />
      <FrameLabel>Не отправилось: «Повторить» доступна с клавиатуры</FrameLabel>
      <Message markdown="Купить билеты на субботу" sentAt={at(16, 10)} status="failed" onRetry={() => undefined} />
      <FrameLabel>Не отправилось, подсвечено, длинный текст</FrameLabel>
      <Message
        markdown="Купить билеты на субботу, позвонить маме, разобрать фотографии из похода и ещё раз перечитать письмо от Ивана перед встречей."
        sentAt={at(16, 11)}
        status="failed"
        highlighted
        onRetry={() => undefined}
      />
    </>
  ),
};

/** Keyboard focus on "Retry": the focus ring is visible on the bubble fill. */
export const Фокус: Story = {
  render: () => <FocusedRetry />,
};

/** Puts focus on the "Retry" button after display: this is how the story shows the focus ring without a mouse. */
function FocusedRetry() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector("button")?.focus(), []);
  return (
    <div ref={host}>
      <Message markdown="Купить билеты на субботу" sentAt={at(16, 10)} status="failed" onRetry={() => undefined} />
    </div>
  );
}

/** Hover, a short message: to the right of the bubble, level with the bottom line, the action row (copy, edit, task, favorite, more). "Copy" works, the others are only visual. */
export const Наведение: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} showActions />
    </div>
  ),
};

/** Hover, a message of maximum width (about 65 characters) in a 720 px column: there is still enough room on the right. */
export const НаведениеШирокаяПлашка: Story = {
  render: () => (
    <div className="story-column">
      <Message
        markdown="Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации."
        sentAt={at(9, 12)}
        showActions
      />
    </div>
  ),
};

/** Hover in a narrow window: there is no room on the right, the row stands over the top right corner of the bubble itself and covers only its edge. */
export const НаведениеУзкоеОкно: Story = {
  render: () => (
    <div className="story-column story-column--narrow">
      <Message
        markdown="Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft."
        sentAt={at(9, 12)}
        showActions
      />
    </div>
  ),
};

/** Hover on a message in a series: the row to the right of the bubble touches neither the upper nor the lower neighboring bubble. */
export const НаведениеВСерии: Story = {
  render: () => (
    <div className="story-column" style={{ gap: "var(--layout-message-gap-series)" }}>
      <Message markdown="Первая мысль" sentAt={at(9, 12)} />
      <Message markdown="Вторая, через две минуты" sentAt={at(9, 14)} showActions />
      <Message markdown="Третья, ещё через три" sentAt={at(9, 17)} />
    </div>
  ),
};

/** Keyboard focus on the first button of the row: the row is visible, the button has a focus ring. */
export const ФокусНаДействиях: Story = {
  render: () => <FocusedActions />,
};

function FocusedActions() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector<HTMLButtonElement>(".message-actions button")?.focus(), []);
  return (
    <div className="story-column" ref={host}>
      <Message markdown="Купить билеты на субботу" sentAt={at(16, 10)} />
    </div>
  );
}

/** Hover on the first message at the top edge of the feed in a narrow window: there is no room above the bubble, the row is pressed inside instead of being clipped by the feed edge. */
export const НаведениеПервоеСообщение: Story = {
  render: () => (
    <div className="story-column story-column--narrow story-feed-top">
      <div className="feed-scroll">
        <ol className="feed">
          <li className="feed__item">
            <Message markdown="Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft." sentAt={at(9, 12)} showActions />
          </li>
        </ol>
      </div>
    </div>
  ),
};

/** Hover in a series in a narrow window: the gap is 3.25 px, the row above the bubble inevitably overlaps the bottom edge of the previous one (and its time, if the bubbles have the same width). */
export const НаведениеВСерииУзкоеОкно: Story = {
  render: () => (
    <div className="story-column story-column--narrow" style={{ gap: "var(--layout-message-gap-series)" }}>
      <Message markdown="Первая мысль, достаточно длинная, чтобы занять почти всю ширину" sentAt={at(9, 12)} />
      <Message markdown="Вторая, через две минуты, тоже довольно длинная для узкой колонки" sentAt={at(9, 14)} showActions />
    </div>
  ),
};
