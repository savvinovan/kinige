import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { IconButton } from "../controls/IconButton";
import type { IconName } from "../icons/Icon";

/** Сколько «Скопировано» остаётся в подписи кнопки, прежде чем вернуться к «Копировать». */
const COPIED_MS = 2000;

type CopyState = "idle" | "copied" | "failed";

const COPY_KEY = { idle: "message.copy", copied: "message.copied", failed: "message.copyFailed" } as const;

/** Остальные действия в D1 только нарисованы: слайс 3 (контекстное меню и флаги задача / избранное), редактирование — позже. */
const VIEW_ONLY = [
  { icon: "edit", key: "message.edit" },
  { icon: "task", key: "message.task" },
  { icon: "favorite", key: "message.favorite" },
  { icon: "more", key: "message.more" },
] as const satisfies readonly { icon: IconName; key: string }[];

/**
 * Ряд действий справа от плашки, на уровне её нижней строки (рядом со временем): копировать, редактировать, задача,
 * избранное, «ещё». Виден при наведении на сообщение и когда фокус внутри него (стили в message.css); в узком окне,
 * где справа нет места, Message ставит его поверх правого верхнего угла самой плашки. Это панель инструментов с одной остановкой в обходе по Tab: стрелки,
 * Home и End ходят между кнопками. «Копировать» работает — `navigator.clipboard.writeText` текста сообщения (Markdown),
 * новых разрешений Tauri не нужно; остальные пока только вид (`aria-disabled`, «Появится позже»).
 */
export function MessageActions({ markdown }: { markdown: string }) {
  const { t } = useTranslation();
  const [copy, setCopy] = useState<CopyState>("idle");
  const [active, setActive] = useState(0);
  const bar = useRef<HTMLDivElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(timer.current), []);

  const copyText = async () => {
    let next: CopyState = "copied";
    try {
      await navigator.clipboard.writeText(markdown);
    } catch {
      // Буфер обмена недоступен (нет разрешения, не защищённый контекст): честно говорим об этом, а не делаем вид.
      next = "failed";
    }
    setCopy(next);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopy("idle"), COPIED_MS);
  };

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

  return (
    // Внешний слой — место справа от плашки вместе с прозрачным зазором (мышь переходит с плашки на ряд, не теряя наведения);
    // внутренний — сам ряд. Ширину внешнего слоя Message сравнивает со свободным местом колонки.
    <div className="message-actions">
      <div className="message-actions__bar" role="toolbar" aria-label={t("message.actions")} ref={bar} onKeyDown={onKeyDown}>
      <IconButton
        small
        icon={copy === "copied" ? "done" : "copy"}
        label={t(COPY_KEY[copy])}
        tabIndex={active === 0 ? 0 : -1}
        onFocus={() => setActive(0)}
        onClick={() => void copyText()}
      />
      {VIEW_ONLY.map((action, index) => (
        <IconButton
          small
          unavailable
          key={action.key}
          icon={action.icon}
          label={t(action.key)}
          tabIndex={active === index + 1 ? 0 : -1}
          onFocus={() => setActive(index + 1)}
        />
      ))}
      <span className="visually-hidden" role="status">
        {copy === "idle" ? "" : t(COPY_KEY[copy])}
      </span>
      </div>
    </div>
  );
}
