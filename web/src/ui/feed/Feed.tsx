import { useMemo, type PointerEvent } from "react";
import { useTranslation } from "react-i18next";
import { useLanguage } from "../../i18n/I18n";
import { Message } from "../message/Message";
import type { MoveTarget } from "../message/moveTargets";
import { type FeedMessage, buildFeed } from "./buildFeed";
import { formatDayLabel } from "./dates";
import "./feed.css";

/** Действия над записями ленты; каждое получает id записи. Без них плашки ведут себя как в D1 (кнопки нарисованы и недоступны). */
export interface FeedActions {
  onEdit(id: string): void;
  onSaveEdit(id: string, text: string): void;
  onCancelEdit(id: string): void;
  onToggleTask(id: string, on: boolean): void;
  onToggleDone(id: string, done: boolean): void;
  onToggleFavorite(id: string, on: boolean): void;
  onChangeDate(id: string): void;
  onDelete(id: string): void;
  onRestore(id: string): void;
  onToggleTaskItem(id: string, index: number, checked: boolean): void;
  onTagClick(tag: string): void;
  /** «Переместить в чат» (меню записи и перетаскивание на чат). */
  onMoveTo(id: string, chatId: string): void;
}

/** Состояние интерфейса записей: какая правится, какие удалены (стоят приглушёнными на месте до ухода из чата). */
export interface FeedUiState {
  editingId?: string | null;
  deletedIds?: ReadonlySet<string>;
  /** Ошибка сохранения правки (уже переведённая). */
  editError?: string;
}

interface FeedProps {
  messages: readonly FeedMessage[];
  /** Сегодняшний `journal_date` (`YYYY-MM-DD`) для меток «Сегодня» и «Вчера»: его присылает Rust. В историях фиксируется, чтобы снимки были стабильны. */
  today: string;
  onRetry?: (messageId: string) => void;
  /** Щелчок по ссылке в записи → системный браузер. */
  onOpenLink?: (url: string) => void;
  ui?: FeedUiState;
  actions?: FeedActions;
  /** Текст пустого состояния; по умолчанию — подсказка пустого чата. */
  emptyText?: string;
  /** Чаты для пункта «Переместить в чат»; текущий помечен (`useRecordDrag` и `moveTargetsFrom`). */
  moveTargets?: readonly MoveTarget[];
  /** Перетаскивание плашки на чат: `begin(id)` — обработчик нажатия плашки. Без него плашки не тащатся. */
  onDragStart?: (messageId: string) => (event: PointerEvent<HTMLElement>, plate: HTMLElement) => void;
}

/**
 * Лента: метки дней («таблетки» по центру) и плашки сообщений, прижатые влево. Открывается прокрученной к низу
 * (новое внизу): область прокрутки `column-reverse` держит нижний край на месте, даже когда после загрузки шрифтов
 * меняется высота строк; короткая лента лежит у нижнего края, над composer. Над лентой нет ни заголовка, ни названия чата (§6).
 */
export function Feed({ messages, today, onRetry, onOpenLink, ui, actions, emptyText, moveTargets, onDragStart }: FeedProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const items = useMemo(() => buildFeed(messages), [messages]);
  const labels = { today: t("feed.today"), yesterday: t("feed.yesterday") };

  return (
    // Прокручиваемая область берёт фокус с клавиатуры, иначе без мыши ленту не пролистать.
    <div className="feed-scroll" tabIndex={0} role="region" aria-label={t("feed.label")}>
      {items.length === 0 ? (
        <p className="feed__empty">{emptyText ?? t("feed.empty")}</p>
      ) : (
        <ol className="feed">
          {items.map((item) =>
            item.kind === "day" ? (
              <li className="feed__item feed__item--day" key={item.key}>
                <h2 className="day-label">{formatDayLabel(item.journalDate, today, language, labels)}</h2>
              </li>
            ) : (
              <li className={`feed__item${item.inSeries ? " feed__item--series" : ""}`} key={item.key}>
                <Message
                  markdown={item.message.bodyMd}
                  sentAt={new Date(item.message.createdAt)}
                  timeZone={item.message.createdTz}
                  status={item.message.status}
                  highlighted={item.message.highlighted}
                  onRetry={onRetry ? () => onRetry(item.message.id) : undefined}
                  onOpenLink={onOpenLink}
                  isTask={item.message.isTask}
                  taskDone={item.message.taskDoneAt != null}
                  favorite={item.message.favoriteAt != null}
                  readOnly={item.message.readOnly}
                  tags={item.message.tags}
                  mode={
                    ui?.editingId === item.message.id ? "editing" : ui?.deletedIds?.has(item.message.id) ? "deleted" : "view"
                  }
                  editError={ui?.editingId === item.message.id ? ui?.editError : undefined}
                  moveTargets={moveTargets}
                  onDragStart={onDragStart?.(item.message.id)}
                  {...(actions ? messageHandlers(actions, item.message.id) : {})}
                />
              </li>
            ),
          )}
        </ol>
      )}
    </div>
  );
}

/** Обработчики плашки по id записи. */
function messageHandlers(actions: FeedActions, id: string) {
  return {
    onEdit: () => actions.onEdit(id),
    onSaveEdit: (text: string) => actions.onSaveEdit(id, text),
    onCancelEdit: () => actions.onCancelEdit(id),
    onToggleTask: (on: boolean) => actions.onToggleTask(id, on),
    onToggleDone: (done: boolean) => actions.onToggleDone(id, done),
    onToggleFavorite: (on: boolean) => actions.onToggleFavorite(id, on),
    onChangeDate: () => actions.onChangeDate(id),
    onDelete: () => actions.onDelete(id),
    onRestore: () => actions.onRestore(id),
    onToggleTaskItem: (index: number, checked: boolean) => actions.onToggleTaskItem(id, index, checked),
    onTagClick: actions.onTagClick,
    onMoveTo: (chatId: string) => actions.onMoveTo(id, chatId),
  };
}
