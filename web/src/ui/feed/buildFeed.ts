import { continuesSeries } from "./dates";

/** Сообщение для ленты: поля настоящего ответа Rust (`MessageView`) плюс два состояния вида из D1. */
export interface FeedMessage {
  id: string;
  /** UTC, мс. */
  createdAt: number;
  /** IANA-пояс, в котором запись создана: в нём показывается время. */
  createdTz: string;
  /** День записи (`YYYY-MM-DD`) — по нему метки дней и серии; считает Rust (граница дня 04:00). */
  journalDate: string;
  /** Текст в Markdown. */
  bodyMd: string;
  /** «failed» — плашка «Не отправилось» (в приложении слайса 1 её нет: при ошибке текст остаётся в поле; состояние осталось в Storybook). */
  status?: "sent" | "failed";
  highlighted?: boolean;
  /** Запись — задача (флажок у плашки). */
  isTask?: boolean;
  /** Когда задача выполнена (мс); `null` или нет — не выполнена. */
  taskDoneAt?: number | null;
  /** Когда добавлена в избранное (мс). */
  favoriteAt?: number | null;
  /** Теги записи в нижнем регистре, без `#` (разбирает Rust): по ним подсвечивается текст. */
  tags?: readonly string[];
  /** Запись новее поддерживаемой версии: только для чтения, действия недоступны. */
  readOnly?: boolean;
  /** Кто создал запись (`core.origin`): `agent` — тихая пометка «добавлено агентом» в строке времени. */
  origin?: string;
}

export type FeedItem =
  | { kind: "day"; key: string; journalDate: string }
  | { kind: "message"; key: string; message: FeedMessage; /** Продолжает серию: промежуток сверху меньше. */ inSeries: boolean };

/** Порядок ленты и курсора (§4.1 ТЗ): `(journal_date, created_at, id)`. */
function byFeedOrder(a: FeedMessage, b: FeedMessage): number {
  if (a.journalDate !== b.journalDate) return a.journalDate < b.journalDate ? -1 : 1;
  if (a.createdAt !== b.createdAt) return a.createdAt - b.createdAt;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

/**
 * Лента: метка дня перед первым сообщением каждого `journal_date`, у каждого сообщения признак серии (пауза < 5 минут,
 * тот же день). Порядок — `(journal_date, created_at, id)`, как у Rust; слияния в «страницы» нет, у каждого своё время (ТЗ §6).
 */
export function buildFeed(messages: readonly FeedMessage[]): FeedItem[] {
  const sorted = [...messages].sort(byFeedOrder);
  const items: FeedItem[] = [];
  let previous: FeedMessage | undefined;
  for (const message of sorted) {
    if (!previous || previous.journalDate !== message.journalDate) {
      items.push({ kind: "day", key: `day-${message.journalDate}`, journalDate: message.journalDate });
    }
    items.push({
      kind: "message",
      key: message.id,
      message,
      inSeries: previous !== undefined && continuesSeries(previous, message),
    });
    previous = message;
  }
  return items;
}
