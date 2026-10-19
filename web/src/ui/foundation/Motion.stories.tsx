import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { motionDurations } from "../tokenSource";
import "./foundation.css";
import "./motion.css";

const EASINGS = [
  { name: "standard", about: "по умолчанию", duration: "medium" },
  { name: "enter", about: "появление", duration: "long" },
  { name: "exit", about: "уход", duration: "long" },
] as const;

/** Четыре числа из значения вида «cubic-bezier(0.2, 0, 0, 1)» (CSS-переменная --motion-easing-*). */
function bezierPoints(name: string): [number, number, number, number] {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(`--motion-easing-${name}`);
  const numbers = raw.match(/-?\d*\.?\d+/g)?.map(Number) ?? [0, 0, 1, 1];
  return [numbers[0] ?? 0, numbers[1] ?? 0, numbers[2] ?? 1, numbers[3] ?? 1];
}

function Curve({ name }: { name: string }) {
  const [x1, y1, x2, y2] = bezierPoints(name);
  const scale = 100;
  const d = `M0,${scale} C${x1 * scale},${scale - y1 * scale} ${x2 * scale},${scale - y2 * scale} ${scale},0`;
  return (
    <svg className="motion__curve" viewBox="-5 -5 110 110" role="img" aria-label={`Кривая ${name}`}>
      <path d={d} />
    </svg>
  );
}

function Track({ label, dotClass, atEnd }: { label: string; dotClass: string; atEnd: boolean }) {
  return (
    <div className="motion__row">
      <span>{label}</span>
      <div className="motion__track">
        <span className={`motion__dot ${dotClass}${atEnd ? " motion__dot--end" : ""}`} />
      </div>
    </div>
  );
}

function Demo() {
  const [atEnd, setAtEnd] = useState(false);
  return (
    <div className="motion">
      <button type="button" className="demo-button" onClick={() => setAtEnd((value) => !value)}>
        Проиграть
      </button>
      {Object.entries(motionDurations).map(([name, value]) => (
        <Track key={name} label={`duration.${name}: ${value.normal}`} dotClass={`motion__dot--${name}`} atEnd={atEnd} />
      ))}
      {EASINGS.map((easing) => (
        <div className="motion__row" key={easing.name}>
          <span>
            easing.{easing.name}: {easing.about}
          </span>
          <div>
            <Curve name={easing.name} />
          </div>
        </div>
      ))}
      <Track label="easing.enter, long" dotClass="motion__dot--enter" atEnd={atEnd} />
      <Track label="easing.exit, long" dotClass="motion__dot--exit" atEnd={atEnd} />
    </div>
  );
}

function Motion() {
  return (
    <main className="foundation">
      <h1 className="foundation__title">Анимации</h1>
      <p className="foundation__lead">
        Три длительности и три кривые. При Reduce Motion (настройка ОС <code>prefers-reduced-motion</code> или атрибут{" "}
        <code>data-reduce-motion</code>) все длительности становятся нулевыми: состояние меняется мгновенно. Компоненты
        берут только <code>--motion-duration-*</code> и <code>--motion-easing-*</code>.
      </p>
      <div className="foundation__grid">
        <section className="candidate" aria-labelledby="motion-normal">
          <h2 className="candidate__name" id="motion-normal">
            Обычный режим
          </h2>
          <p className="candidate__about">Нажмите «Проиграть».</p>
          <Demo />
        </section>
        <section className="candidate" data-reduce-motion aria-labelledby="motion-reduced">
          <h2 className="candidate__name" id="motion-reduced">
            Reduce Motion
          </h2>
          <p className="candidate__about">
            Те же дорожки при <code>data-reduce-motion</code>:{" "}
            {Object.entries(motionDurations)
              .map(([name, value]) => `${name} ${value.normal} → ${value.reduced}`)
              .join(", ")}
            .
          </p>
          <Demo />
        </section>
      </div>
    </main>
  );
}

const meta = {
  title: "Основа/Анимации",
  component: Motion,
} satisfies Meta<typeof Motion>;

export default meta;

export const Длительности: StoryObj<typeof meta> = {};
