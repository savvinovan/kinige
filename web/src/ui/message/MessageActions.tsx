import { useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../controls/IconButton";
import type { IconName } from "../icons/Icon";
import { positionBelow, type MenuPosition } from "../menu/ContextMenu";
import type { CopyState } from "./useCopy";

const COPY_KEY = { idle: "message.copy", copied: "message.copied", failed: "message.copyFailed" } as const;

interface MessageActionsProps {
  copyState: CopyState;
  onCopy: () => void;
  isTask: boolean;
  favorite: boolean;
  /** Запись только для чтения (группа новее поддерживаемой версии): всё, кроме «Копировать», недоступно. */
  readOnly: boolean;
  /** Обработчики без аргумента: «включить / выключить» решает Message по текущему состоянию. Нет обработчика — кнопка только нарисована. */
  onEdit?: () => void;
  onToggleTask?: () => void;
  onToggleFavorite?: () => void;
  /** «Ещё»: открыть контекстное меню у кнопки. */
  onMore: (position: MenuPosition) => void;
  menuOpen: boolean;
}

/**
 * Ряд действий справа от плашки, на уровне её нижней строки (рядом со временем): копировать, редактировать, задача,
 * избранное, «ещё». Виден при наведении на сообщение, пока открыто его меню и когда фокус внутри него (стили в message.css);
 * в узком окне, где справа нет места, Message ставит его поверх правого верхнего угла самой плашки. Это панель инструментов
 * с одной остановкой в обходе по Tab: стрелки, Home и End ходят между кнопками.
 *
 * Состояния: у задачи значок «задача» акцентный, у избранного звезда залитая; подпись для диктора меняется («В избранное» ↔
 * «Убрать из избранного»). Кнопка без обработчика только нарисована (`aria-disabled`, «Появится позже»); у записи только для
 * чтения — недоступна с пояснением, почему.
 */
export function MessageActions({
  copyState,
  onCopy,
  isTask,
  favorite,
  readOnly,
  onEdit,
  onToggleTask,
  onToggleFavorite,
  onMore,
  menuOpen,
}: MessageActionsProps) {
  const { t } = useTranslation();
  const [active, setActive] = useState(0);
  const bar = useRef<HTMLDivElement>(null);

  const buttons = () => [...(bar.current?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
  const move = (to: number) => {
    const list = buttons();
    const index = (to + list.length) % list.length;
    setActive(index);
    list[index]?.focus();
  };
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const current = buttons().indexOf(document.activeElement as HTMLButtonElement);
    const keys: Record<string, number> = { ArrowRight: current + 1, ArrowLeft: current - 1, Home: 0, End: -1 };
    if (!(event.key in keys)) return;
    event.preventDefault();
    move(keys[event.key] ?? 0);
  };

  // Кнопка действия над записью: нет обработчика — «Появится позже», запись только для чтения — недоступна с пояснением.
  const action = (index: number, icon: IconName, label: string, handler: (() => void) | undefined, on = false) => (
    <IconButton
      small
      icon={icon}
      label={label}
      className={on ? `message-actions__on${icon === "favorite" ? " message-actions__filled" : ""}` : undefined}
      unavailable={!handler && !readOnly}
      {...(readOnly ? { "aria-disabled": true, title: `${label}. ${t("error.read_only")}` } : {})}
      tabIndex={active === index ? 0 : -1}
      onFocus={() => setActive(index)}
      onClick={() => {
        if (!readOnly) handler?.();
      }}
    />
  );

  return (
    // Внешний слой — место справа от плашки вместе с прозрачным зазором (мышь переходит с плашки на ряд, не теряя наведения);
    // внутренний — сам ряд. Ширину внешнего слоя Message сравнивает со свободным местом колонки.
    <div className="message-actions">
      <div className="message-actions__bar" role="toolbar" aria-label={t("message.actions")} ref={bar} onKeyDown={onKeyDown}>
        <IconButton
          small
          icon={copyState === "copied" ? "done" : "copy"}
          label={t(COPY_KEY[copyState])}
          tabIndex={active === 0 ? 0 : -1}
          onFocus={() => setActive(0)}
          onClick={onCopy}
        />
        {action(1, "edit", t("message.edit"), onEdit)}
        {action(2, "task", isTask ? t("message.untask") : t("message.task"), onToggleTask, isTask)}
        {action(3, "favorite", favorite ? t("message.unfavorite") : t("message.favorite"), onToggleFavorite, favorite)}
        <IconButton
          small
          icon="more"
          label={t("message.more")}
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          tabIndex={active === 4 ? 0 : -1}
          onFocus={() => setActive(4)}
          onClick={(event) => onMore(positionBelow(event.currentTarget))}
        />
        <span className="visually-hidden" role="status">
          {copyState === "idle" ? "" : t(COPY_KEY[copyState])}
        </span>
      </div>
    </div>
  );
}
