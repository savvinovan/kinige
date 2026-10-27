import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import type { ShortcutStatus } from "../../api/generated/ShortcutStatus";
import { Button } from "../controls/Button";
import { acceleratorFromEvent, formatAccelerator, parseAccelerator, type Modifier } from "./accelerator";
import { StatusNote } from "./StatusNote";
import "./shortcut-row.css";

interface ShortcutRowProps {
  status: ShortcutStatus;
  /** Уже переведённый текст ошибки сохранения; показывается под строкой как `role="alert"` вместо состояния «занято». */
  error?: string;
  /** Записано новое сочетание (каноничная строка `Ctrl+Alt+Shift+Cmd+<код>`). */
  onChange: (accelerator: string) => void;
  /** «Сбросить»: вернуть сочетание по умолчанию (или попробовать назначить его снова, если оно занято). */
  onReset: () => void;
  /** Режим записи включился или закончился (Rust на это время отключает сами глобальные сочетания, иначе нажатие не дойдёт до окна). */
  onRecording: (active: boolean) => void;
}

/**
 * Строка «Сочетание»: название действия, сочетание клавишами-«капсулами» (⌥ Space), кнопки «Изменить» и «Сбросить»,
 * под ними состояние: «Не назначено: занято…» или ошибка сохранения.
 *
 * Запись нового сочетания: «Изменить» → вместо капсул поле «Нажмите новое сочетание…», фокус в нём, слушается `keydown`
 * с отменой действия браузера. Принимаются только клавиши из списка (`acceleratorFromEvent`); остальное, включая Tab,
 * Enter и стрелки, игнорируется, и запись продолжается. Esc отменяет, потеря фокуса тоже. Окно настроек при записи Esc
 * и Tab не получает (`stopPropagation`): Esc выходит из записи, а не закрывает окно. Режим объявляется диктору (`role="status"`).
 */
export function ShortcutRow({ status, error, onChange, onReset, onRecording }: ShortcutRowProps) {
  const { t } = useTranslation();
  const nameId = useId();
  const [recording, setRecording] = useState(false);
  const recorder = useRef<HTMLDivElement>(null);
  const actions = useRef<HTMLDivElement>(null);
  const returnFocus = useRef(false);
  // Свежий обработчик без переподписки: родитель может передавать новую функцию на каждый рендер.
  const onRecordingRef = useRef(onRecording);
  useEffect(() => {
    onRecordingRef.current = onRecording;
  });

  // Начало и конец записи (в том числе размонтирование и потеря фокуса) сообщаются родителю одним местом.
  useEffect(() => {
    if (!recording) return;
    onRecordingRef.current(true);
    recorder.current?.focus();
    return () => onRecordingRef.current(false);
  }, [recording]);

  // После записи фокус возвращается на «Изменить», чтобы с клавиатуры не потерять место (кнопки на время записи скрыты).
  useEffect(() => {
    if (recording || !returnFocus.current) return;
    returnFocus.current = false;
    actions.current?.querySelector("button")?.focus();
  }, [recording]);

  // Запись заканчивается один раз: потеря фокуса из-за самого закрытия поля записи не должна отменить возврат фокуса на кнопку.
  const active = useRef(false);
  const start = () => {
    active.current = true;
    setRecording(true);
  };
  const stop = (refocus: boolean) => {
    if (!active.current) return;
    active.current = false;
    returnFocus.current = refocus;
    setRecording(false);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    // Всё, что нажато в режиме записи, принадлежит записи: ни Tab, ни Esc не доходят до окна настроек.
    event.preventDefault();
    event.stopPropagation();
    if (event.nativeEvent.isComposing) return;
    if (event.key === "Escape" && !event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey) {
      stop(true);
      return;
    }
    const accelerator = acceleratorFromEvent(event);
    if (accelerator === null) return;
    stop(true);
    onChange(accelerator);
  };

  const name = t(`settings.shortcuts.${status.kind}`);
  const taken = status.state === "taken";
  const { modifiers, key } = parseAccelerator(status.accelerator);
  const spoken = [...modifiers.map((modifier: Modifier) => t(`settings.shortcuts.modifier.${modifier.toLowerCase() as Lowercase<Modifier>}`)), ...(key ? [key] : [])].join(" + ");

  return (
    <div className="shortcut-row" role="group" aria-labelledby={nameId}>
      <span className="shortcut-row__name" id={nameId}>
        {name}
      </span>
      <div className="shortcut-row__line">
        {recording ? (
          <div
            className="shortcut-row__recorder"
            ref={recorder}
            tabIndex={0}
            role="group"
            aria-label={t("settings.shortcuts.recordingLabel", { name })}
            onKeyDown={onKeyDown}
            onBlur={() => stop(false)}
          >
            <span aria-hidden="true">{t("settings.shortcuts.recording")}</span>
          </div>
        ) : (
          <span
            className={`shortcut-row__keys${taken ? " shortcut-row__keys--taken" : ""}`}
            role="img"
            aria-label={t("settings.shortcuts.keys", { keys: spoken })}
          >
            {formatAccelerator(status.accelerator).map((cap, index) => (
              <kbd className="keycap" key={`${index}-${cap}`} aria-hidden="true">
                {cap}
              </kbd>
            ))}
          </span>
        )}
        <div className={`shortcut-row__actions${recording ? " shortcut-row__actions--hidden" : ""}`} ref={actions}>
          <Button variant="secondary" aria-label={t("settings.shortcuts.changeLabel", { name })} onClick={start}>
            {t("settings.shortcuts.change")}
          </Button>
          {!status.isDefault || taken ? (
            <Button variant="accent" aria-label={t("settings.shortcuts.resetLabel", { name })} onClick={onReset}>
              {t("settings.shortcuts.reset")}
            </Button>
          ) : null}
        </div>
      </div>
      {/* Объявление режима записи: область есть всегда, а текст появляется с началом записи, иначе диктор его не услышит.
          Без role="status": у окна настроек уже есть строки состояния, и это не одна из них. */}
      <span className="visually-hidden" aria-live="polite" aria-atomic="true">
        {recording ? t("settings.shortcuts.recording") : ""}
      </span>
      {error ? (
        <StatusNote tone="error" icon="alert" role="alert">
          {error}
        </StatusNote>
      ) : taken ? (
        <StatusNote tone="error" icon="alert" role="status">
          {t("settings.shortcuts.taken")}
        </StatusNote>
      ) : null}
    </div>
  );
}
