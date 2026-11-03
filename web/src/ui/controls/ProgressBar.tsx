import "./controls.css";

interface ProgressBarProps {
  /** Done. */
  value: number;
  /** Total; with `0` the bar is empty. */
  max: number;
  /** The bar name for the screen reader. */
  label: string;
  /** The value text for the screen reader ("3 200 из 15 000"); without it the screen reader reads percentages. */
  valueText?: string;
}

/**
 * A progress bar: a quiet line, an accent fill going left to right. This is `role="progressbar"`; the fill width changes with
 * a short animation from the motion token, immediately under Reduce Motion.
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
