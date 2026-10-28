import { useId, type InputHTMLAttributes, type ReactNode } from "react";
import "./controls.css";

interface CheckboxProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "children"> {
  /** Видимая подпись справа от квадрата; она же даёт имя флажку диктору. */
  children: ReactNode;
  /** Вторая строка тихим цветом под подписью (сколько записей, сколько уже есть). */
  description?: ReactNode;
}

/**
 * Флажок: настоящий `<input type="checkbox">` (пробел, фокус, диктор работают сами), нарисован квадратом задачи из ленты —
 * серая рамка, отмеченный — акцентная заливка с галочкой. Кольцо фокуса — только с клавиатуры. Подпись обрезается
 * многоточием, если не помещается.
 */
export function Checkbox({ children, description, className, ...rest }: CheckboxProps) {
  const descriptionId = useId();
  const labelId = useId();
  return (
    <label className={`checkbox${className ? ` ${className}` : ""}`}>
      <input
        type="checkbox"
        className="checkbox__input"
        aria-labelledby={labelId}
        aria-describedby={description ? descriptionId : undefined}
        {...rest}
      />
      <span className="checkbox__box" aria-hidden="true" />
      <span className="checkbox__text">
        <span className="checkbox__label" id={labelId}>
          {children}
        </span>
        {description ? (
          <span className="checkbox__description" id={descriptionId}>
            {description}
          </span>
        ) : null}
      </span>
    </label>
  );
}
