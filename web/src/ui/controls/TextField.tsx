import { useId, type InputHTMLAttributes } from "react";
import { Icon, type IconName } from "../icons/Icon";
import "./controls.css";

interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "aria-label" | "aria-invalid"> {
  /** Имя поля для диктора (видимой подписи нет: смысл виден из значка и подсказки). */
  label: string;
  /** Значок слева внутри поля. */
  icon?: IconName;
  /** Текст ошибки: показывается под полем, поле помечается недопустимым. */
  error?: string;
}

/** Однострочное поле ввода (поиск и подобное). Рамка — `color.border.control`, фокус — общее кольцо `:focus-visible`. */
export function TextField({ label, icon, error, className, ...rest }: TextFieldProps) {
  const errorId = useId();
  return (
    <div className={`text-field${className ? ` ${className}` : ""}`}>
      <label className={`text-field__box${error ? " text-field__box--invalid" : ""}`}>
        {icon ? <Icon name={icon} /> : null}
        <input
          className="text-field__input"
          aria-label={label}
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? errorId : undefined}
          {...rest}
        />
      </label>
      {error ? (
        <p className="text-field__error" id={errorId}>
          {error}
        </p>
      ) : null}
    </div>
  );
}
