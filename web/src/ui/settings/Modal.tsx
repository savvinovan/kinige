import { useEffect, useId, useRef, useState, type ReactNode, type RefObject } from "react";
import { IconButton } from "../controls/IconButton";
import "./modal.css";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

interface ModalProps {
  /** Заголовок окна; на него указывает `aria-labelledby`. */
  title: string;
  /** Подпись кнопки закрытия (для диктора и подсказки). */
  closeLabel: string;
  onClose: () => void;
  /**
   * Куда вернуть фокус при закрытии. По умолчанию — элемент, который был в фокусе в момент открытия; если его к тому времени
   * убрали со страницы (пункт закрывшегося меню), вернуть фокус некуда, и родителю стоит передать триггер здесь.
   */
  returnFocusRef?: RefObject<HTMLElement | null>;
  /**
   * Окно можно закрыть: крестик, Esc и щелчок по подложке. Без этого (идущий импорт) закрыть окно нечем, кроме кнопок
   * внутри него; Tab ходит по кругу как обычно.
   */
  closable?: boolean;
  children: ReactNode;
}

// `tabindex="-1"` выводит элемент из обхода Tab (выбранный вариант радиогруппы — единственная остановка группы),
// хотя селектор кнопок его захватывает: такие элементы отбрасываются.
const focusablesIn = (root: HTMLElement) =>
  [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((element) => element.getAttribute("tabindex") !== "-1");

/**
 * Модальное окно внутри главного окна приложения (не второе окно ОС): подложка, панель по центру, заголовок и кнопка
 * закрытия, прокручиваемое содержимое. Без нативного `<dialog>`: `showModal` есть не везде (в jsdom нет), а нам нужна
 * проверяемая и одинаковая в WKWebView работа клавиатуры.
 *
 * Клавиатура и фокус:
 * - при открытии фокус уходит на первый управляющий элемент (кнопку закрытия), при закрытии возвращается на открывший элемент;
 * - Tab и Shift+Tab ходят по кругу внутри окна (переход считает само окно, а не браузер); фокус, попавший наружу, возвращается;
 * - Esc и щелчок по подложке закрывают окно.
 *
 * Появление — короткое затухание и сдвиг; при Reduce Motion длительности нулевые (themes.css), окно появляется сразу.
 */
export function Modal({ title, closeLabel, onClose, returnFocusRef, closable = true, children }: ModalProps) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);
  // Открывший элемент запоминается до первой отрисовки панели, пока фокус ещё на нём.
  const [opener] = useState(() => (document.activeElement instanceof HTMLElement ? document.activeElement : null));
  // Свежий обработчик без переподписки на события: родитель может передавать новую функцию на каждый рендер.
  const onCloseRef = useRef(onClose);
  const closableRef = useRef(closable);
  useEffect(() => {
    onCloseRef.current = onClose;
    closableRef.current = closable;
  });

  useEffect(() => {
    const root = panel.current;
    if (!root) return;
    (focusablesIn(root)[0] ?? root).focus();

    // Окно над окном (предупреждение над «Доступом ИИ-агентов»): клавиатуру и фокус ведёт только верхнее.
    const isTopmost = () => {
      const panels = document.querySelectorAll(".modal__panel");
      return panels[panels.length - 1] === root;
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (!isTopmost()) return;
      if (event.key === "Escape") {
        if (event.defaultPrevented) return;
        event.preventDefault();
        if (closableRef.current) onCloseRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      event.preventDefault();
      const items = focusablesIn(root);
      if (items.length === 0) {
        root.focus();
        return;
      }
      const index = items.indexOf(document.activeElement as HTMLElement);
      const next = event.shiftKey ? (index <= 0 ? items.length - 1 : index - 1) : index === items.length - 1 ? 0 : index + 1;
      items[next]?.focus();
    };
    // Фокус не должен уйти за окно ни щелчком, ни программно.
    const onFocusIn = (event: FocusEvent) => {
      if (isTopmost() && event.target instanceof Node && !root.contains(event.target)) (focusablesIn(root)[0] ?? root).focus();
    };
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("focusin", onFocusIn);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("focusin", onFocusIn);
      // Ref родителя читается именно в момент закрытия: нужен элемент, который стоит там сейчас, а не при открытии.
      // eslint-disable-next-line react-hooks/exhaustive-deps
      const target = returnFocusRef?.current ?? opener;
      if (target?.isConnected) target.focus();
    };
  }, [opener, returnFocusRef]);

  return (
    <div className="modal">
      <div className="modal__scrim" aria-hidden="true" onClick={closable ? onClose : undefined} />
      <div className="modal__panel" role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1} ref={panel}>
        <header className="modal__header">
          <h2 className="modal__title" id={titleId}>
            {title}
          </h2>
          {closable ? <IconButton icon="close" label={closeLabel} onClick={onClose} /> : null}
        </header>
        <div className="modal__body">{children}</div>
      </div>
    </div>
  );
}
