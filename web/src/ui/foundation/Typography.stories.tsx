import type { Meta, StoryObj } from "@storybook/react-vite";
import { active, fontsets } from "../tokenSource";
import "./foundation.css";
import "./typography.css";

type Role = "sans" | "mono";

const FACES: { role: Role; weight: "regular" | "semibold" | "bold"; italic?: boolean; label: string; text: string }[] = [
  { role: "sans", weight: "regular", label: "Inter · Regular 400", text: "Тихо тянется день. Quiet days, written down. 0123456789 09:12" },
  { role: "sans", weight: "regular", italic: true, label: "Inter · Italic 400", text: "Пишешь как в мессенджере, перечитываешь как книгу. Write like you text." },
  { role: "sans", weight: "semibold", label: "Inter · SemiBold 600", text: "Заголовки и кнопки. Headings and buttons" },
  { role: "sans", weight: "bold", label: "Inter · Bold 700", text: "Жирный в тексте записи. Bold inside a note" },
  { role: "sans", weight: "bold", italic: true, label: "Inter · Bold Italic 700", text: "Жирный курсив. Bold italic emphasis" },
  { role: "mono", weight: "regular", label: "Mono · Regular 400", text: "git rebase --onto main  // Raft: ёё 0O 1lI" },
  { role: "mono", weight: "bold", label: "Mono · Bold 700", text: "let today = Day::new(2026, 10, 20);" },
];

const SCALE: { role: string; label: string; text: string }[] = [
  { role: "day", label: "day: метка дня, 11,2 px", text: "14 марта 2025" },
  { role: "h1", label: "h1: 16,8 px", text: "Заголовок первого уровня" },
  { role: "h2", label: "h2: 14,8 px", text: "Заголовок второго уровня" },
  { role: "h3", label: "h3: 13,9 px", text: "Заголовок третьего уровня" },
  { role: "body", label: "body: текст записи, 13 px", text: "Проснулся рано и до завтрака записал мысль про журнал. Woke up early and wrote a thought down." },
  { role: "small", label: "small: интерфейс, 12,1 px", text: "Входящие, Идеи, Книги, Работа, Рецепты. Search" },
  { role: "code", label: "code: код, 12,1 px", text: "fn main() { println!(\"Записей: {}\", today.count()); }" },
  { role: "meta", label: "meta: время, 10,4 px", text: "09:12" },
];

function Specimen() {
  const name = active.fontset;
  const set = fontsets[name];
  if (!set) return null;
  return (
    <section className="candidate" aria-labelledby={`fontset-${name}`}>
      <h2 className="candidate__name" id={`fontset-${name}`}>
        {name}
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
      <p className="face__sample face__sample--sans face__sample--regular">Лес — это то, что вырастает из записей.</p>
      <p className="face__sample face__sample--sans face__sample--bold">Лес — это то, что вырастает из записей.</p>

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
        <p className="scale__text scale__text--day">Сегодня</p>
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
      <h1 className="foundation__title">Типографика</h1>
      <p className="foundation__lead">
        Набор «Inter + JetBrains Mono» с кириллицей, выбран пользователем (Inter и для текста записей, и для интерфейса). Все начертания — встроенные woff2, не системные.
        Компактная шкала: текст записей 13 px (выбран пользователем), интерфейс 12,1 px, время 10,4 px. Размеры в rem (один rem
        равен размеру текста записей); «Крупный текст» (×1,25) переключается в панели Storybook.
      </p>
      <div className="foundation__grid">
        <Specimen />
      </div>
    </main>
  );
}

const meta = {
  title: "Основа/Типографика",
  component: Typography,
} satisfies Meta<typeof Typography>;

export default meta;

export const Начертания: StoryObj<typeof meta> = {};
