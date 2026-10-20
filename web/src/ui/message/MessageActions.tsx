import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { IconButton } from "../controls/IconButton";
import type { IconName } from "../icons/Icon";

/** Сколько «Скопировано» остаётся в подписи кнопки, прежде чем вернуться к «Копировать». */
const COPIED_MS = 2000;

type CopyState = "idle" | "copied" | "failed";

const COPY_LABEL: Record<CopyState, string> = {
  idle: "Копировать",
  copied: "Скопировано",
  failed: "Не удалось скопировать",
};

/** Остальные действия в D1 только нарисованы: слайс 3 (контекстное меню и флаги задача / избранное), редактирование — позже. */
const VIEW_ONLY: { icon: IconName; label: string }[] = [
  { icon: "edit", label: "Редактировать" },
  { icon: "task", label: "Сделать задачей" },
  { icon: "favorite", label: "В избранное" },
  { icon: "more", label: "Ещё" },
];

/**
 * Ряд действий под плашкой: копировать, редактировать, задача, избранное, «ещё». Виден при наведении на сообщение и
 * когда фокус внутри него (стили в message.css). Это панель инструментов с одной остановкой в обходе по Tab: стрелки,
 * Home и End ходят между кнопками. «Копировать» работает — `navigator.clipboard.writeText` текста сообщения (Markdown),
 * новых разрешений Tauri не нужно; остальные пока только вид (`aria-disabled`, «Появится позже»).
 */
export function MessageActions({ markdown }: { markdown: string }) {
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
    <div className="message-actions" role="toolbar" aria-label="Действия с сообщением" ref={bar} onKeyDown={onKeyDown}>
      <IconButton
        small
        icon={copy === "copied" ? "done" : "copy"}
        label={COPY_LABEL[copy]}
        tabIndex={active === 0 ? 0 : -1}
        onFocus={() => setActive(0)}
        onClick={() => void copyText()}
      />
      {VIEW_ONLY.map((action, index) => (
        <IconButton
          small
          unavailable
          key={action.label}
          icon={action.icon}
          label={action.label}
          tabIndex={active === index + 1 ? 0 : -1}
          onFocus={() => setActive(index + 1)}
        />
      ))}
      <span className="visually-hidden" role="status">
        {copy === "idle" ? "" : COPY_LABEL[copy]}
      </span>
    </div>
  );
}
