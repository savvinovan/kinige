import type { Meta, StoryObj } from "@storybook/react-vite";
import { contrastRatio } from "../contrast";
import { CONTRAST_PAIRS, MIN_RATIO } from "../contrastPairs";
import { PALETTE_ROLES, active, palettes } from "../tokenSource";
import "./foundation.css";
import "./colors.css";

const cssVar = (role: string) => `var(--color-${role.replaceAll(".", "-")})`;

function PaletteSheet({ name }: { name: string }) {
  const palette = palettes[name];
  if (!palette) return null;
  return (
    <section className="candidate" aria-labelledby={`palette-${name}`}>
      <h2 className="candidate__name" id={`palette-${name}`}>
        {name}
      </h2>
      <p className="candidate__about">{palette.description}</p>

      <ul className="swatches" aria-label={`Роли цвета: ${name}`}>
        {PALETTE_ROLES.map((role) => (
          <li className="swatch" key={role}>
            <span className="swatch__chip" style={{ background: cssVar(role) }} role="img" aria-label={role} />
            <span>{role}</span>
            <span className="swatch__value">{palette.colors[role]}</span>
            {palette.origins[role] ? <span className="swatch__origin">{palette.origins[role]}</span> : null}
          </li>
        ))}
      </ul>

      <div className="sample">
        <p className="sample__day">Сегодня</p>
        <div className="sample__plate">
          <p className="sample__text">
            Проснулся рано и до завтрака записал мысль про журнал: <em>пишешь как в мессенджере, перечитываешь как
            книгу</em>. Главное — <strong>не выбирать</strong>, куда писать.
          </p>
          <p className="sample__text sample__secondary">Вторичный текст: подпись, имя чата.</p>
          <p className="sample__text">
            <span className="sample__link">The Psychology of Habits — fs.blog</span>{" "}
            <code className="sample__code">git rebase --onto main</code>
          </p>
          <p className="sample__meta">09:12</p>
        </div>
        <div className="sample__plate sample__plate--highlighted">
          <p className="sample__text">Подсвеченная запись: нашлась поиском. Заливка чуть светлее и бирюзовее.</p>
          <p className="sample__meta">09:14</p>
        </div>
        <div className="sample__plate">
          <p className="sample__text">Купить билеты на субботу</p>
          <p className="sample__error">Не отправилось. Повторить</p>
        </div>
        <span className="sample__focus">Фокус с клавиатуры</span>
      </div>

      <table className="ratios">
        <caption className="foundation__label">Контраст пар (WCAG 2.x)</caption>
        <thead>
          <tr>
            <th scope="col">Цвет</th>
            <th scope="col">на</th>
            <th scope="col" className="ratios__number">
              Контраст
            </th>
            <th scope="col">Норма</th>
          </tr>
        </thead>
        <tbody>
          {CONTRAST_PAIRS.map((pair) => {
            const ratio = contrastRatio(palette.colors[pair.foreground] ?? "", palette.colors[pair.background] ?? "");
            const min = MIN_RATIO[pair.kind];
            return (
              <tr key={`${pair.foreground}/${pair.background}/${pair.kind}`}>
                <td>{pair.foreground}</td>
                <td>{pair.background}</td>
                <td className="ratios__number">{ratio.toFixed(2)}:1</td>
                <td className="ratios__norm">
                  {ratio >= min ? "ок" : "НИЖЕ НОРМЫ"} ≥ {min}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}

function Colors() {
  return (
    <main className="foundation">
      <h1 className="foundation__title">Цвета: нейтральный графит</h1>
      <p className="foundation__lead">
        Единственная палитра (ADR-0004): серые поверхности и текст без оттенка, акцент и ошибка из Nord. Под ролью
        указано происхождение цвета: «авторский», официальное имя в палитре Nord или «подгонка». Компоненты знают только роли (<code>color.surface.message</code>, <code>color.text.primary</code> …),
        поэтому смена палитры — смена значений в <code>design/tokens/palettes</code>. Контраст всех пар проверяет тест{" "}
        <code>tokens.test.ts</code>.
      </p>
      <div className="foundation__grid">
        <PaletteSheet name={active.palette} />
      </div>
    </main>
  );
}

const meta = {
  title: "Основа/Цвета",
  component: Colors,
  parameters: { docs: { description: { component: "Роли палитры, образец плашек и контраст всех пар." } } },
} satisfies Meta<typeof Colors>;

export default meta;

export const Палитра: StoryObj<typeof meta> = {};
