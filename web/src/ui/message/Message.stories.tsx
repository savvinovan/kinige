import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef } from "react";
import "../feed/feed.css";
import { Frame, FrameLabel } from "../stories/Frame";
import { Message } from "./Message";

// Время в историях фиксировано (местное), чтобы снимки не менялись от запуска к запуску.
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

/** Обычное: короткий текст — короткая плашка, время в правом нижнем углу. */
export const Обычное: Story = {};

/** Курсив, жирный, ссылка и строчный код в одном абзаце; плашка по тексту, но не шире ~80 знаков. */
export const Текст: Story = {
  args: {
    markdown:
      "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать. Потом [прочитал про привычки](https://fs.blog/habits) и починил историю командой `git rebase --onto main`.",
  },
};

/** Заголовки H1–H3: ненамного крупнее текста (компактная шкала). */
export const Заголовки: Story = {
  render: () => (
    <>
      <Message markdown={"# Заголовок первого уровня\nТекст под ним."} sentAt={at(8, 40)} />
      <Message markdown={"## Заголовок второго уровня\nТекст под ним."} sentAt={at(8, 41)} />
      <Message markdown={"### Планы на неделю\nЗакончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда."} sentAt={at(8, 42)} />
    </>
  ),
};

/** Списки и задачи: обычные пункты, нумерованный список, отмеченные и неотмеченные задачи. */
export const СпискиИЗадачи: Story = {
  render: () => (
    <>
      <Message markdown={"### Купить\n- хлеб\n- молоко\n- кофе"} sentAt={at(10, 0)} />
      <Message markdown={"1. Проснуться\n2. Позавтракать\n3. Записать мысль"} sentAt={at(10, 1)} />
      <Message markdown={"### Планы на выходные\n- [x] Поход в горы\n- [ ] Разобрать фотографии\n- [x] Почитать книгу"} sentAt={at(11, 30)} />
    </>
  ),
};

/** Цитата: черта только у цитаты, следующий абзац без черты. */
export const Цитата: Story = {
  args: {
    markdown: "> Лучше сделано, чем идеально.\n\nЗаписал, чтобы перечитать, когда начну полировать пятый день подряд.",
    sentAt: at(21, 5),
  },
};

/** Код: строчный и блок; блок на отдельной заливке, время под блоком. */
export const Код: Story = {
  args: {
    markdown: `Починил историю командой \`git rebase --onto main\`, вот что получилось:\n\n${CODE}`,
    sentAt: at(13, 45),
  },
};

/** Код с длинной строкой и абзац после него (KINIG-40): строка переносится, под блоком нет полосы прокрутки, и первая строка абзаца ничем не закрыта, даже если ОС показывает полосы всегда. */
export const КодСДлиннойСтрокой: Story = {
  args: {
    markdown: `Вот команда сборки:\n\n${LONG_CODE}\n\nПосле неё запускаю проверки и жду зелёного результата.`,
    sentAt: at(14, 10),
  },
};

/** Длинный текст: плашка упирается в ширину ~80 знаков и переносит строки; время остаётся у конца последней. */
export const ДлинныйТекст: Story = {
  args: {
    markdown:
      "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации, и записать вопросы, которые остались после чтения. Потом вернуться к этой записи через неделю и проверить, на сколько удалось ответить.",
    sentAt: at(9, 12),
  },
};

/** Длинная строка без пробелов и длинная строка кода: обе переносятся, прокрутки нет. */
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

/** Кириллица и латиница в одном сообщении, цифры, пунктуация, тире. */
export const КириллицаИЛатиница: Story = {
  args: {
    markdown:
      "Съешь же ещё этих мягких французских булок, да выпей чаю. The quick brown fox jumps over the lazy dog. 0123456789 — «ёлочки», “quotes”, ёЁ.",
  },
};

/** Состояния: подсвечено, не отправилось, оба вместе. Серию (меньший промежуток) показывает история «Лента». */
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

/** Фокус с клавиатуры на «Повторить»: кольцо фокуса видно на заливке плашки. */
export const Фокус: Story = {
  render: () => <FocusedRetry />,
};

/** Ставит фокус на кнопку «Повторить» после показа: так история показывает кольцо фокуса без мыши. */
function FocusedRetry() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => host.current?.querySelector("button")?.focus(), []);
  return (
    <div ref={host}>
      <Message markdown="Купить билеты на субботу" sentAt={at(16, 10)} status="failed" onRetry={() => undefined} />
    </div>
  );
}

/** Наведение, короткое сообщение: справа от плашки, на уровне нижней строки, ряд действий (копировать, редактировать, задача, избранное, ещё). «Копировать» работает, остальные только вид. */
export const Наведение: Story = {
  render: () => (
    <div className="story-column">
      <Message markdown="Купить хлеб" sentAt={at(9, 12)} showActions />
    </div>
  ),
};

/** Наведение, сообщение максимальной ширины (около 65 знаков) в колонке 720 px: справа ещё хватает места. */
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

/** Наведение в узком окне: справа нет места, ряд стоит поверх правого верхнего угла самой плашки и накрывает только её край. */
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

/** Наведение на сообщение в серии: ряд справа от плашки не задевает ни верхнюю, ни нижнюю соседнюю плашку. */
export const НаведениеВСерии: Story = {
  render: () => (
    <div className="story-column" style={{ gap: "var(--layout-message-gap-series)" }}>
      <Message markdown="Первая мысль" sentAt={at(9, 12)} />
      <Message markdown="Вторая, через две минуты" sentAt={at(9, 14)} showActions />
      <Message markdown="Третья, ещё через три" sentAt={at(9, 17)} />
    </div>
  ),
};

/** Фокус с клавиатуры на первой кнопке ряда: ряд виден, у кнопки кольцо фокуса. */
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

/** Наведение на первое сообщение у верхнего края ленты в узком окне: над плашкой нет места, ряд прижимается внутрь, а не обрезается краем ленты. */
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

/** Наведение в серии в узком окне: промежуток 3,25 px, ряд над плашкой неизбежно заходит на нижний край предыдущей (и на её время, если плашки одной ширины). */
export const НаведениеВСерииУзкоеОкно: Story = {
  render: () => (
    <div className="story-column story-column--narrow" style={{ gap: "var(--layout-message-gap-series)" }}>
      <Message markdown="Первая мысль, достаточно длинная, чтобы занять почти всю ширину" sentAt={at(9, 12)} />
      <Message markdown="Вторая, через две минуты, тоже довольно длинная для узкой колонки" sentAt={at(9, 14)} showActions />
    </div>
  ),
};
