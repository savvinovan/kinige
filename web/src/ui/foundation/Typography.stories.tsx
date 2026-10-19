import type { Meta, StoryObj } from "@storybook/react-vite";
import { active, fontsets } from "../tokenSource";
import "./foundation.css";
import "./typography.css";

type Role = "serif" | "sans" | "mono";

const FACES: { role: Role; weight: "regular" | "semibold" | "bold"; italic?: boolean; label: string; text: string }[] = [
  { role: "serif", weight: "regular", label: "Serif · Regular 400", text: "Тихо тянется день. Quiet days, written down. 0123456789" },
  { role: "serif", weight: "regular", italic: true, label: "Serif · Italic 400", text: "Пишешь как в мессенджере, перечитываешь как книгу. Write like you text." },
  { role: "serif", weight: "semibold", label: "Serif · SemiBold 600", text: "Заголовок дня и подзаголовки. Headings and subheads" },
  { role: "serif", weight: "bold", label: "Serif · Bold 700", text: "Жирный в тексте записи. Bold inside a note" },
  { role: "sans", weight: "regular", label: "Sans · Regular 400", text: "Время, подписи и интерфейс. Timestamps and UI 09:12" },
  { role: "sans", weight: "semibold", label: "Sans · SemiBold 600", text: "Кнопки и акценты интерфейса. Buttons and emphasis" },
  { role: "sans", weight: "bold", label: "Sans · Bold 700", text: "Сильный акцент. Strong emphasis" },
  { role: "mono", weight: "regular", label: "Mono · Regular 400", text: "git rebase --onto main  // Raft: ёё 0O 1lI" },
  { role: "mono", weight: "bold", label: "Mono · Bold 700", text: "let today = Day::new(2026, 10, 20);" },
];

const SCALE: { role: string; label: string; text: string }[] = [
  { role: "day", label: "day: заголовок дня", text: "Вторник, 20 октября" },
  { role: "h1", label: "h1", text: "Заголовок первого уровня" },
  { role: "h2", label: "h2", text: "Заголовок второго уровня" },
  { role: "h3", label: "h3", text: "Заголовок третьего уровня" },
  { role: "body", label: "body: текст записи", text: "Проснулся рано и до завтрака записал мысль про журнал. Woke up early and wrote a thought down." },
  { role: "small", label: "small: подсказки и подписи", text: "Enter — отправить, Shift+Enter — новая строка" },
  { role: "code", label: "code: код", text: "fn main() { println!(\"Записей: {}\", today.count()); }" },
  { role: "meta", label: "meta: время и счётчики", text: "09:12 · 3 записи" },
];

function Candidate({ name }: { name: string }) {
  const set = fontsets[name];
  if (!set) return null;
  return (
    <section className="candidate" data-fontset={name} aria-labelledby={`fontset-${name}`}>
      <h2 className="candidate__name" id={`fontset-${name}`}>
        {name}
        {name === active.fontset ? " (по умолчанию)" : ""}
      </h2>
      <p className="candidate__about">{set.description}</p>

      <h3 className="foundation__heading">Начертания</h3>
      <div className="faces">
        {FACES.map((face) => (
          <p className="scale__row" key={face.label}>
            <span className="face__label">{face.label}</span>
            <span
              className={`face__sample face__sample--${face.role} face__sample--${face.weight}${face.italic ? " face__sample--italic" : ""}`}
            >
              {face.text}
            </span>
          </p>
        ))}
      </div>

      <h3 className="foundation__heading">Regular рядом с Bold</h3>
      <p className="face__sample face__sample--serif face__sample--regular">Лес — это то, что вырастает из записей.</p>
      <p className="face__sample face__sample--serif face__sample--bold">Лес — это то, что вырастает из записей.</p>

      <h3 className="foundation__heading">Шкала</h3>
      <div className="scale">
        {SCALE.map((row) => (
          <p className="scale__row" key={row.role}>
            <span className="face__label">{row.label}</span>
            <span className={`scale__text scale__text--${row.role}`}>{row.text}</span>
          </p>
        ))}
      </div>

      <h3 className="foundation__heading">Образец записи</h3>
      <div className="page">
        <p className="scale__text scale__text--day">Вторник, 20 октября</p>
        <p className="scale__text scale__text--meta page__time">09:12</p>
        <p className="scale__text scale__text--body page__paragraph">
          Проснулся рано и до завтрака записал мысль про журнал: <em>пишешь как в мессенджере, перечитываешь как
          книгу</em>. Главное — <strong>не выбирать</strong>, куда писать. Потом открыл <code className="page__code">git rebase --onto main</code> и
          разобрался с историей: <i lang="en">the quick brown fox jumps over the lazy dog</i>.
        </p>
        <p className="scale__text scale__text--body page__paragraph">
          Ещё: позвонить маме, купить хлеб, дописать заметку про Raft и распределённые транзакции — Съешь же ещё этих
          мягких французских булок, да выпей чаю. Длинное слово без пробелов: сверхъестественнонепроизносимоеслово.
        </p>
      </div>
    </section>
  );
}

function Typography() {
  return (
    <main className="foundation">
      <h1 className="foundation__title">Типографика: кандидаты шрифтов</h1>
      <p className="foundation__lead">
        Три набора «serif + sans + mono» с кириллицей. Все начертания — встроенные woff2, не системные. Шкала одна на все
        наборы: размеры в rem, масштаб текста переключается в панели Storybook («Крупный текст»).
      </p>
      <div className="foundation__grid">
        {Object.keys(fontsets)
          .sort((a, b) => Number(b === active.fontset) - Number(a === active.fontset) || a.localeCompare(b))
          .map((name) => (
            <Candidate key={name} name={name} />
          ))}
      </div>
    </main>
  );
}

const meta = {
  title: "Основа/Типографика",
  component: Typography,
} satisfies Meta<typeof Typography>;

export default meta;

export const Кандидаты: StoryObj<typeof meta> = {};
