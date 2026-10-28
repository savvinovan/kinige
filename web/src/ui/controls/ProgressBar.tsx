import "./controls.css";

interface ProgressBarProps {
  /** Сделано. */
  value: number;
  /** Всего; при `0` полоса пуста. */
  max: number;
  /** Имя полосы для диктора. */
  label: string;
  /** Текст значения для диктора («3 200 из 15 000»); без него диктор читает проценты. */
  valueText?: string;
}

/**
 * Полоса прогресса: тихая линия, акцентная заливка слева направо. Это `role="progressbar"`; ширина заливки меняется с
 * короткой анимацией из токена движения, при Reduce Motion — сразу.
 */
export function ProgressBar({ value, max, label, valueText }: ProgressBarProps) {
  const safeMax = Math.max(max, 0);
  const safeValue = Math.min(Math.max(value, 0), safeMax);
  const share = safeMax === 0 ? 0 : (safeValue / safeMax) * 100;
  return (
    <div
      className="progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={safeMax}
      aria-valuenow={safeValue}
      aria-valuetext={valueText}
    >
      <div className="progress__fill" style={{ width: `${share}%` }} />
    </div>
  );
}
