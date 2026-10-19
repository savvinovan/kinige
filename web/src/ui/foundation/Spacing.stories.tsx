import type { Meta, StoryObj } from "@storybook/react-vite";
import "./foundation.css";
import "./spacing.css";

const SPACES = ["1", "2", "3", "4", "5", "6", "7", "8"];

/** Значение из tokens.css (rem) для подписи; читается из активной темы. */
const tokenValue = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

const SHAPES = [
  { token: "radius-inline", label: "radius.inline", about: "строчный код, тег", className: "shape__box--inline" },
  { token: "radius-control", label: "radius.control", about: "кнопки и поля", className: "shape__box--control" },
  { token: "radius-panel", label: "radius.panel", about: "блок кода, composer, тост", className: "shape__box--panel" },
  { token: "radius-round", label: "radius.round", about: "полностью скруглённое", className: "shape__box--round" },
  { token: "shadow-raised", label: "shadow.raised", about: "composer, тост", className: "shape__box--raised" },
  { token: "shadow-overlay", label: "shadow.overlay", about: "меню, диалог", className: "shape__box--overlay" },
];

function Spacing() {
  return (
    <main className="foundation">
      <h1 className="foundation__title">Отступы, радиусы, тени</h1>
      <p className="foundation__lead">
        Шкала отступов с шагом 4 px при масштабе текста 100%; заданы в rem, поэтому растут вместе с текстом, и крупный
        текст не слипается. Разделитель — линия <code>border.width.hairline</code> цвета <code>color.border.divider</code>,
        фокус — кольцо <code>focus.width</code> с зазором <code>focus.offset</code>.
      </p>

      <h2 className="foundation__heading">space.*</h2>
      <ul className="spaces" aria-label="Шкала отступов">
        {SPACES.map((step) => (
          <li className="space" key={step}>
            <span>space.{step}</span>
            <span className="foundation__label">
              {tokenValue(`--space-${step}`)} = {Number.parseFloat(tokenValue(`--space-${step}`)) * 16} px при 100%
            </span>
            <span className="space__bar" style={{ width: `var(--space-${step})` }} />
          </li>
        ))}
      </ul>

      <h2 className="foundation__heading">Радиусы и тени</h2>
      <div className="shapes">
        {SHAPES.map((shape) => (
          <figure className="shape" key={shape.token}>
            <div className={`shape__box ${shape.className}`} />
            <figcaption className="foundation__label">
              {shape.label}: {shape.about}
            </figcaption>
          </figure>
        ))}
      </div>
    </main>
  );
}

const meta = {
  title: "Основа/Отступы",
  component: Spacing,
} satisfies Meta<typeof Spacing>;

export default meta;

export const Шкала: StoryObj<typeof meta> = {};
