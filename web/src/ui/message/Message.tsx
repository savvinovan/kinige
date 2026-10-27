import { useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { Button } from "../controls/Button";
import { formatTime } from "../feed/dates";
import { Icon } from "../icons/Icon";
import { positionBelow, type MenuPosition } from "../menu/ContextMenu";
import { renderMarkdown } from "./markdown";
import { MessageActions } from "./MessageActions";
import { MessageEditor } from "./MessageEditor";
import { MessageMenu } from "./MessageMenu";
import { selectedTextIn, useCopy } from "./useCopy";
import { useActionsPlacement } from "./useActionsPlacement";
import "./message.css";
import "./markdown.css";

export interface MessageProps {
  /** Текст сообщения в Markdown. Сырой HTML в нём не рендерится (§6). */
  markdown: string;
  /** Когда отправлено; время показывается в 24-часовом формате в поясе записи (`timeZone`), без него — в поясе окна. */
  sentAt: Date;
  /** IANA-пояс, в котором запись создана (`created_tz`). */
  timeZone?: string;
  /** «failed» — не отправилось: под текстом строка «Не отправилось. Повторить». */
  status?: "sent" | "failed";
  /** Подсвеченное сообщение (найдено поиском, переход по ссылке): другая заливка плашки. */
  highlighted?: boolean;
  onRetry?: () => void;
  /** Щелчок по ссылке в тексте: адрес уходит в системный браузер (команда `open_external_url`). Без обработчика ссылка ничего не открывает. */
  onOpenLink?: (url: string) => void;
  /** Показать ряд действий без наведения (для историй и снимков состояния «наведение»). */
  showActions?: boolean;

  // Слайс 3. Все свойства необязательны; без обработчиков поведение D1: кнопки нарисованы и недоступны.
  /** Запись — задача: перед текстом флажок. */
  isTask?: boolean;
  /** Задача выполнена: флажок отмечен, текст приглушён. */
  taskDone?: boolean;
  /** В избранном: тихая звезда рядом со временем. */
  favorite?: boolean;
  /** Запись создана более новой версией (группа в неподдерживаемой `sv`): действия недоступны, копировать можно. */
  readOnly?: boolean;
  /** Теги записи (нижний регистр, без `#`), их определил Rust: в тексте они подсвечиваются и нажимаются. */
  tags?: readonly string[];
  /**
   * view — обычная; editing — текст заменён полем правки; deleted — приглушена, вместо времени «Удалено · Вернуть»,
   * ряда действий и меню нет (решение 1 слайса 3).
   */
  mode?: "view" | "editing" | "deleted";
  /** Начать правку (кнопка в ряду, пункт меню). Режим `editing` включает родитель. */
  onEdit?: () => void;
  /** Enter в поле правки: непустой текст. */
  onSaveEdit?: (text: string) => void;
  /** Esc в поле правки. */
  onCancelEdit?: () => void;
  /** Уже переведённая ошибка сохранения правки: строка `role="alert"` внутри плашки. */
  editError?: string;
  /** Пункт «Сделать задачей» / «Убрать задачу»: `on` — новое состояние. */
  onToggleTask?: (on: boolean) => void;
  /** Щелчок по флажку задачи: `done` — новое состояние. */
  onToggleDone?: (done: boolean) => void;
  onToggleFavorite?: (on: boolean) => void;
  onChangeDate?: () => void;
  onDelete?: () => void;
  onRestore?: () => void;
  /** Щелчок по `- [ ]` внутри текста: `index` — порядковый номер квадрата в плашке (с нуля), `checked` — новое состояние. */
  onToggleTaskItem?: (index: number, checked: boolean) => void;
  /** Щелчок по тегу: тег в нижнем регистре, без `#`. */
  onTagClick?: (tag: string) => void;
}

/**
 * Сообщение — плашка без хвоста и аватара; время мелко в правом нижнем углу на одной строке с концом текста.
 * Промежуток до соседних плашек задаёт лента (серия — меньше), здесь только сама плашка.
 *
 * Слайс 3: ряд действий и контекстное меню (правый щелчок, «ещё», Shift+F10 или клавиша меню на любом элементе плашки),
 * правка в плашке, флажок задачи, отметка «избранное», удалённая плашка, кликабельные теги и квадраты `- [ ]`.
 * Компонент «глупый»: состояние (режим, флаги) и обработчики приходят снаружи.
 */
export function Message({
  markdown,
  sentAt,
  timeZone,
  status = "sent",
  highlighted = false,
  onRetry,
  onOpenLink,
  showActions = false,
  isTask = false,
  taskDone = false,
  favorite = false,
  readOnly = false,
  tags,
  mode = "view",
  onEdit,
  onSaveEdit,
  onCancelEdit,
  editError,
  onToggleTask,
  onToggleDone,
  onToggleFavorite,
  onChangeDate,
  onDelete,
  onRestore,
  onToggleTaskItem,
  onTagClick,
}: MessageProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const failed = status === "failed";
  const viewing = mode === "view";
  const labelDone = t("message.taskDone");
  const labelTodo = t("message.taskTodo");
  const labelItem = t("message.taskItem");
  const interactiveTasks = viewing && !readOnly && onToggleTaskItem !== undefined;
  const tagsKey = (tags ?? []).join(" ");
  const html = useMemo(
    () =>
      renderMarkdown(markdown, {
        timeSpacer: !failed,
        taskLabels: { done: labelDone, todo: labelTodo, item: labelItem },
        tags: tagsKey === "" ? undefined : tagsKey.split(" "),
        interactiveTasks,
      }),
    [markdown, failed, labelDone, labelTodo, labelItem, tagsKey, interactiveTasks],
  );
  // Сообщение кончается блоком (код): распорки у последней строки нет, время стоит под блоком.
  const timeBelow = !failed && !html.includes("message__spacer");

  const plate = useRef<HTMLElement>(null);
  const { state: copyState, copy } = useCopy();
  const [menu, setMenu] = useState<{ position: MenuPosition; selection: string | null } | null>(null);

  // Ряд действий: справа от плашки; в узком окне — чуть выше плашки у её правого края; у верхнего края ленты — внутри.
  // Виден, пока мышь в строке ленты (и ещё короткую задержку после ухода). Подробности — в useActionsPlacement.
  const { wrap, inside, flip, shown } = useActionsPlacement(`${html}|${mode}`);

  // Ширина плашки до правки: пока плашка обычная, её ширина запоминается (и обновляется при изменении размера); при входе
  // в правку она закрепляется переменной `--message-edit-width`, так что плашка меняет только высоту.
  const viewWidth = useRef(0);
  useLayoutEffect(() => {
    const element = plate.current;
    if (!element || mode === "editing") return;
    viewWidth.current = element.offsetWidth;
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      viewWidth.current = element.offsetWidth;
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [mode, html]);
  useLayoutEffect(() => {
    const element = plate.current;
    if (!element || mode !== "editing") return;
    if (viewWidth.current > 0) element.style.setProperty("--message-edit-width", `${viewWidth.current}px`);
    return () => {
      element.style.removeProperty("--message-edit-width");
    };
  }, [mode]);

  const openMenu = (position: MenuPosition) => {
    if (viewing) setMenu({ position, selection: selectedTextIn(plate.current) });
  };
  const copyNow = (selection: string | null) => void copy(selection ?? markdown);

  // Webview не уходит по ссылке: щелчок мышью, Enter на ссылке с клавиатуры (браузер превращает его в click) и средняя
  // кнопка (auxclick) отменяются; адрес открывает системный браузер через `open_external_url` (проверка схемы — в Rust).
  const handleLinkClick = (event: MouseEvent<HTMLDivElement>) => {
    if (!(event.target instanceof Element)) return;
    const link = event.target.closest("a");
    if (!link) return;
    event.preventDefault();
    const href = link.getAttribute("href");
    // Среднюю кнопку не открываем: её щелчок «отменяется» как переход, а открытие — только по основному (click).
    if (event.type === "click" && href) onOpenLink?.(href);
  };

  // Квадрат `- [ ]` в тексте и тег: нажимаются мышью, Enter и пробелом (это span с role, а не настоящие кнопки).
  const activate = (target: Element, root: HTMLElement): boolean => {
    const box = target.closest(".task__box");
    if (box && interactiveTasks) {
      const index = [...root.querySelectorAll(".task__box")].indexOf(box);
      onToggleTaskItem?.(index, box.getAttribute("aria-checked") !== "true");
      return true;
    }
    const tag = target.closest(".tag");
    if (tag && onTagClick) {
      onTagClick((tag.textContent ?? "").replace(/^#/, "").toLowerCase());
      return true;
    }
    return false;
  };
  const handleBodyClick = (event: MouseEvent<HTMLDivElement>) => {
    handleLinkClick(event);
    if (event.target instanceof Element) activate(event.target, event.currentTarget);
  };
  const handleBodyKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== " " && event.key !== "Enter") return;
    if (event.target instanceof Element && activate(event.target, event.currentTarget)) event.preventDefault();
  };

  // Меню с клавиатуры: Shift+F10 или клавиша меню на любом элементе плашки (кнопка ряда, флажок, ссылка) открывает его под ним.
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!viewing || !(event.target instanceof Element)) return;
    if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
      event.preventDefault();
      openMenu(positionBelow(event.target));
    }
  };

  const toggle = (handler: ((on: boolean) => void) | undefined, current: boolean) =>
    handler ? () => handler(!current) : undefined;
  const checkLocked = !viewing || readOnly || !onToggleDone;

  const classes = ["message"];
  if (highlighted) classes.push("message--highlighted");
  if (timeBelow) classes.push("message--time-below");
  if (isTask) classes.push("message--task");
  if (isTask && taskDone) classes.push("message--task-done");
  if (favorite && mode !== "deleted") classes.push("message--favorite");
  if (mode === "editing") classes.push("message--editing");
  if (mode === "deleted") classes.push("message--deleted");

  return (
    <>
      <div
        className={[
          "message-wrap",
          showActions || shown || menu ? "message-wrap--actions" : "",
          inside ? "message-wrap--actions-inside" : "",
          inside && flip ? "message-wrap--actions-flip" : "",
        ]
          .filter(Boolean)
          .join(" ")}
        ref={wrap}
        onContextMenu={(event) => {
          if (!viewing) return;
          event.preventDefault();
          openMenu({ x: event.clientX, y: event.clientY });
        }}
        onKeyDown={handleKeyDown}
      >
        <article className={classes.join(" ")} ref={plate}>
          {isTask ? (
            <button
              type="button"
              role="checkbox"
              className="message__check"
              aria-checked={taskDone}
              aria-disabled={checkLocked || undefined}
              aria-label={t("message.taskCheck")}
              onClick={() => {
                if (!checkLocked) onToggleDone?.(!taskDone);
              }}
            />
          ) : null}
          <div className="message__content">
            {mode === "editing" ? (
              <MessageEditor initial={markdown} onSave={(text) => onSaveEdit?.(text)} onCancel={() => onCancelEdit?.()} />
            ) : (
              <div
                className="markdown"
                onClick={handleBodyClick}
                onAuxClick={handleLinkClick}
                onKeyDown={handleBodyKeyDown}
                dangerouslySetInnerHTML={{ __html: html }}
              />
            )}
            {mode === "editing" && editError ? (
              <p className="message__error" role="alert">
                {editError}
              </p>
            ) : null}
            {failed ? (
              <p className="message__error">
                {t("message.notSent")}{" "}
                <Button variant="inline" onClick={onRetry} aria-label={t("message.retryLabel")}>
                  {t("message.retry")}
                </Button>
              </p>
            ) : null}
          </div>
          <div className="message__meta">
            {mode === "deleted" ? (
              <span className="message__deleted">
                {t("message.deleted")} ·{" "}
                <Button variant="inline" onClick={onRestore} aria-label={t("message.restoreLabel")}>
                  {t("message.restore")}
                </Button>
              </span>
            ) : (
              <>
                {favorite ? (
                  <span className="message__mark" role="img" aria-label={t("message.favoriteMark")}>
                    <Icon name="favorite" />
                  </span>
                ) : null}
                <time className="message__time" dateTime={sentAt.toISOString()}>
                  {formatTime(sentAt, language, timeZone)}
                </time>
              </>
            )}
          </div>
        </article>
        {viewing ? (
          <MessageActions
            copyState={copyState}
            onCopy={() => copyNow(selectedTextIn(plate.current))}
            isTask={isTask}
            favorite={favorite}
            readOnly={readOnly}
            onEdit={onEdit}
            onToggleTask={toggle(onToggleTask, isTask)}
            onToggleFavorite={toggle(onToggleFavorite, favorite)}
            onMore={openMenu}
            menuOpen={menu !== null}
          />
        ) : null}
      </div>
      {/* Меню — соседний узел, а не потомок обёртки: события из него (правый щелчок, клавиши) не всплывают в плашку. */}
      {menu && viewing ? (
        <MessageMenu
          position={menu.position}
          isTask={isTask}
          favorite={favorite}
          readOnly={readOnly}
          onEdit={onEdit}
          onToggleTask={toggle(onToggleTask, isTask)}
          onToggleFavorite={toggle(onToggleFavorite, favorite)}
          onChangeDate={onChangeDate}
          onDelete={onDelete}
          onCopy={() => copyNow(menu.selection)}
          onClose={() => setMenu(null)}
        />
      ) : null}
    </>
  );
}
