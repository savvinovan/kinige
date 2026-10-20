import type { Meta, StoryObj } from "@storybook/react-vite";
import { contrastRatio } from "../contrast";
import { CONTRAST_PAIRS, MIN_RATIO } from "../contrastPairs";
import { PALETTE_ROLES, active, palettes } from "../tokenSource";
import "./foundation.css";
import "./colors.css";

const cssVar = (role: string) => `var(--color-${role.replaceAll(".", "-")})`;

function Candidate({ name }: { name: string }) {
  const palette = palettes[name];
  if (!palette) return null;
  return (
    <section className="candidate" data-palette={name} aria-labelledby={`palette-${name}`}>
      <h2 className="candidate__name" id={`palette-${name}`}>
        {name}
        {name === active.palette ? " (по умолчанию)" : ""}
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
        <p className="sample__day">Вторник, 20 октября</p>
        <hr className="sample__rule" />
        <p className="sample__time">09:12</p>
        <p className="sample__text">
          Проснулся рано и до завтрака записал мысль про журнал: <em>пишешь как в мессенджере, перечитываешь как
          книгу</em>. Главное — <strong>не выбирать</strong>, куда писать.
        </p>
        <p className="sample__text sample__secondary">Вторичный текст: подпись, приглушённый синтаксис.</p>
        <p className="sample__text">
          <span className="sample__link">The Psychology of Habits — fs.blog</span>
        </p>
        <p className="sample__text sample__highlight">Подсвеченная запись: нашлась поиском.</p>
        <p className="sample__error">Не отправилось. Повторить</p>
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
              <tr key={`${pair.foreground}/${pair.background}`}>
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
      <h1 className="foundation__title">Цвета: кандидаты палитры</h1>
      <p className="foundation__lead">
        Тёмные палитры на одних и тех же ролях: три «бумажные» тёплые или нейтральные (umber, moss, dusk) и три по мотивам
        готовых схем (nord, catppuccin-mocha, catppuccin-macchiato). У последних под ролью указано происхождение цвета:
        имя в официальной палитре или «подгонка». Компоненты знают только роли (<code>color.surface.paper</code>,{" "}
        <code>color.text.primary</code> …), поэтому выбор палитры — смена значения в <code>design/tokens/active.json</code>.
        Контраст проверяет тест <code>tokens.test.ts</code> для каждой палитры.
      </p>
      <div className="foundation__grid">
        {Object.keys(palettes)
          .sort((a, b) => Number(b === active.palette) - Number(a === active.palette) || a.localeCompare(b))
          .map((name) => (
            <Candidate key={name} name={name} />
          ))}
      </div>
    </main>
  );
}

const meta = {
  title: "Основа/Цвета",
  component: Colors,
  parameters: { docs: { description: { component: "Все кандидаты палитры рядом, с ролями и контрастом." } } },
} satisfies Meta<typeof Colors>;

export default meta;

export const Кандидаты: StoryObj<typeof meta> = {};
